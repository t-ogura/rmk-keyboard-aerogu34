//! Start the external 32 MHz crystal before `main`, and keep the previous
//! run's reset marker.
//!
//! RMK's `keyboard.toml` code generation never sets `hfclk_source`, so
//! embassy-nrf keeps its default of `Internal`. The nRF52840 radio cannot run
//! from the internal HFINT oscillator, and `build_sdc` hangs waiting for a
//! usable HFCLK -- before USB starts, so the board simply never appears.
//! Confirmed on hardware (Cornix TB), and confirmed fixed by this. RMK's own
//! hand-written examples start the crystal; only the generated path misses it.
//!
//! The check is on `HFCLKSTAT` (source and state), not on the started event:
//! the bootloader hands over with the crystal already running, and starting a
//! running crystal raises no new event, so an event wait after clearing it
//! spins to its bound on every boot.
//!
//! The wait is bounded so a board without the crystal boots without BLE rather
//! than hanging here.

/// `POWER.GPREGRET2` as found on entry, before this firmware clears it.
///
/// The previous run leaves a marker there (`status_led.rs` writes 0xA5 once
/// it is up; RMK writes `0xA0 | reason` before rebooting itself). The register
/// survives a soft reset and is cleared by a power-on, brown-out, pin or
/// watchdog reset, so this one byte says whether the last attempt ended by
/// the firmware's own hand. `.uninit`: written here, before RAM init.
#[unsafe(link_section = ".uninit.prev_marker")]
pub static mut PREV_MARKER: core::mem::MaybeUninit<u8> = core::mem::MaybeUninit::uninit();

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __pre_init() {
    {
        let power = ::embassy_nrf::pac::POWER;
        let marker = power.gpregret2().read().gpregret();
        // SAFETY: __pre_init runs alone; the static is read only after RAM init.
        unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(PREV_MARKER).cast::<u8>(), marker) };
        power.gpregret2().write_value(::embassy_nrf::pac::power::regs::Gpregret2(0));
    }

    // Feed the watchdog first. RMK starts a 10 s WDT once every task is up, and
    // an nRF52 WDT keeps running through a reset-button reset -- so on the next
    // boot it is already counting down, with whatever was left of its window,
    // while nothing feeds it until the tasks start. Reloading here restarts the
    // full window; on a WDT that is not running the write does nothing.
    ::embassy_nrf::pac::WDT
        .rr(0)
        .write(|w| w.set_rr(::embassy_nrf::pac::wdt::vals::Rr::Reload));

    let clock = ::embassy_nrf::pac::CLOCK;
    let running = || {
        let stat = clock.hfclkstat().read();
        stat.state() && stat.src() == ::embassy_nrf::pac::clock::vals::HfclkstatSrc::Xtal
    };
    if !running() {
        clock.tasks_hfclkstart().write_value(1);
        let mut spins: u32 = 0;
        while !running() {
            spins += 1;
            if spins > 2_000_000 {
                break;
            }
        }
    }
}
