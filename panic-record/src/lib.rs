//! Keep the panic message across the reset it causes.
//!
//! Packaged as a crate called `panic-probe` because RMK's `#[rmk_central]` /
//! `#[rmk_peripheral]` unconditionally emit `use panic_probe as _;` and the
//! real panic-probe defines its own `#[panic_handler]`; the two cannot both
//! be linked, and the name is the only way to satisfy the import.
//!
//! Without a debug probe a panic on this board is invisible: `panic-probe`
//! prints over RTT that nothing is listening to, then the core stops and the
//! watchdog reboots it a few seconds later, so all that can be seen is the
//! status LED's "watchdog" blue. This handler writes the message into RAM
//! that survives a soft reset, marks `GPREGRET2` with a code of its own
//! ([`PANIC_CODE`], shown as five magenta blinks by `status_led.rs`), and
//! resets at once. The next boot reads the record back and puts it on the
//! log (`usb_log` builds), so a panic costs one reset and leaves its
//! location behind.

#![no_std]

use core::fmt::Write;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;

/// `GPREGRET2` marker of a panic reset: `0xA0 | 0x0F`, past RMK's own reboot
/// reasons (1..=4), read as `Reason::SoftReboot(15)` by the status LED.
pub const PANIC_CODE: u8 = 0x0F;
const MAGIC: u32 = 0x50_4E_43_21; // "PNC!"

#[repr(C)]
struct Record {
    magic: u32,
    len: u32,
    text: [u8; 240],
}

/// `.uninit`: never zeroed by the runtime, so it lives through the reset.
#[unsafe(link_section = ".uninit.panic_record")]
static mut RECORD: MaybeUninit<Record> = MaybeUninit::uninit();

struct Cursor<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl Write for Cursor<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let room = self.buf.len() - self.len;
        let n = s.len().min(room);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    // SAFETY: interrupts are off and nothing else runs; the record is only
    // ever read at boot, before any task, by `take`.
    unsafe {
        let record = &mut *core::ptr::addr_of_mut!(RECORD).cast::<Record>();
        let mut cursor = Cursor {
            buf: &mut record.text,
            len: 0,
        };
        let _ = write!(cursor, "{info}");
        record.len = cursor.len as u32;
        record.magic = MAGIC;
    }
    ::embassy_nrf::pac::POWER
        .gpregret2()
        .write_value(::embassy_nrf::pac::power::regs::Gpregret2((0xA0 | PANIC_CODE) as u32));
    cortex_m::peripheral::SCB::sys_reset()
}

/// The previous run's panic message, if it ended in one; cleared on read.
pub fn take() -> Option<heapless::String<240>> {
    // SAFETY: called once at boot before the record could be written again.
    unsafe {
        let record = &mut *core::ptr::addr_of_mut!(RECORD).cast::<Record>();
        if record.magic != MAGIC {
            return None;
        }
        record.magic = 0;
        let len = (record.len as usize).min(record.text.len());
        let text = core::str::from_utf8(&record.text[..len]).unwrap_or("<invalid utf-8>");
        let mut out = heapless::String::new();
        let _ = out.push_str(text);
        Some(out)
    }
}
