//! Split status, and the cause of the last reset, on the Xiao's RGB LED.
//!
//! Without this there is no way to tell a half that is running but unpaired
//! from one that is not running at all -- both look like a keyboard that does
//! nothing.
//!
//! # Boot: why did it reset?
//!
//! For the first ~3 seconds the LED reports `POWER.RESETREAS`, which the
//! nRF52840 latches across a reset, together with the marker the previous
//! run left in `POWER.GPREGRET2` (read in `__pre_init`, `hfxo.rs`, before
//! it is cleared):
//!
//! | LED | last reset |
//! | --- | --- |
//! | off | power-on or reset pin -- a clean start |
//! | blue | **watchdog** -- something stopped feeding it, i.e. a hang the WDT recycled |
//! | magenta, solid | the previous run was up and ended by a soft reset with no reason recorded |
//! | magenta, N blinks | the previous run **rebooted itself** for reason N: 1 BLE runner stopped, 2 storage unreadable, 3 stale split link, 4 host-requested reset |
//! | white | **CPU lockup** -- a fault inside a fault |
//!
//! # After that: is the split up?
//!
//! | LED | meaning |
//! | --- | --- |
//! | red | the other half is not connected |
//! | green | the other half is connected |
//!
//! Shown for three seconds after the reset reason, and again for three
//! seconds whenever the link comes or goes; dark otherwise. The halves run on
//! LiPos, and a lit LED is a few milliamps -- as much as the rest of the idle
//! keyboard together -- so it only speaks when something changes, as ZMK's
//! rgbled-widget did on this board.
//!
//! # Host connection (right half only)
//!
//! When the BLE profile or its state changes -- a `BT0`..`BT4` key, a host
//! connecting or dropping -- the LED blinks the profile number (profile 0 =
//! one blink) in the colour of the new state, as rgbled-widget did:
//!
//! | colour | BLE state |
//! | --- | --- |
//! | blue | connected to the host |
//! | yellow | advertising, waiting for the host |
//!
//! `Inactive` (USB mode, sleep, the instant after a profile switch) is not
//! shown: a switch always advertises right after, and USB needs no light.
//!
//! The same display serves both halves: the right (central) learns about the
//! split link from `PeripheralConnectedEvent` and the host from
//! `ConnectionStatusChangeEvent`, the left (peripheral) from
//! `CentralConnectedEvent`. The connected-event channels are generated with
//! one subscriber slot and nothing else in this build takes them;
//! `connection_status_change` has its base count raised to 2 in
//! `keyboard.toml` for this subscriber. Watch that budget before adding a
//! subscription: overrunning it panics at startup, which on this firmware
//! means a board that never enumerates.

use embassy_nrf::Peri;
use embassy_nrf::gpio::{AnyPin, Level, Output, OutputDrive};
use embassy_nrf::pac;
use rmk::event::{CentralConnectedEvent, ConnectionStatusChangeEvent, PeripheralConnectedEvent};
use rmk::macros::processor;
use rmk::types::ble::{BleState, BleStatus};

const POLL_MS: u64 = 250;
/// How long to hold the reset-reason colour before switching to link status.
const REASON_TICKS: u32 = (3000 / POLL_MS) as u32;
/// How long the link status stays lit after it changes (and once at boot).
const STATUS_TICKS: u32 = (3000 / POLL_MS) as u32;

#[derive(Clone, Copy, PartialEq)]
enum Reason {
    Clean,
    Watchdog,
    SoftwareReset,
    Lockup,
    /// The previous run's GPREGRET2 marker survived: 0 = it was up and ended
    /// without recording why, 1..=4 = RMK's own reboot reason.
    SoftReboot(u8),
}

/// Written to GPREGRET2 once this firmware is up; RMK overwrites it with
/// `0xA0 | reason` if it reboots itself.
const ALIVE_MARKER: u8 = 0xA5;

/// The Xiao BLE's RGB LED: red P0.26, green P0.30, blue P0.06, common anode
/// (driving a pin low lights that colour).
pub struct XiaoRgb {
    r: Output<'static>,
    g: Output<'static>,
    b: Output<'static>,
}

impl XiaoRgb {
    pub fn new(r: Peri<'static, AnyPin>, g: Peri<'static, AnyPin>, b: Peri<'static, AnyPin>) -> Self {
        Self {
            r: Output::new(r, Level::High, OutputDrive::Standard),
            g: Output::new(g, Level::High, OutputDrive::Standard),
            b: Output::new(b, Level::High, OutputDrive::Standard),
        }
    }

    fn set(&mut self, red: bool, green: bool, blue: bool) {
        if red { self.r.set_low() } else { self.r.set_high() }
        if green { self.g.set_low() } else { self.g.set_high() }
        if blue { self.b.set_low() } else { self.b.set_high() }
    }
}

/// A blink sequence announcing a change: `blinks` flashes of `colour`, 250 ms
/// on / 250 ms off, then a 500 ms pause.
#[derive(Clone, Copy)]
struct Notice {
    colour: (bool, bool, bool),
    blinks: u8,
    start: u32,
}

impl Notice {
    fn end(&self) -> u32 {
        self.start + 2 * self.blinks as u32 + 2
    }
}

/// The display logic shared by both halves.
struct Display {
    led: XiaoRgb,
    linked: bool,
    reason: Reason,
    ticks: u32,
    /// Tick at which the current status display goes dark.
    status_until: u32,
    notice: Option<Notice>,
}

impl Display {
    fn new(led: XiaoRgb) -> Self {
        // RESETREAS is sticky: bits accumulate until written back. Read it once,
        // then clear it so the next boot reports its own cause rather than the
        // union of every cause so far.
        let raw = pac::POWER.resetreas().read().0;
        pac::POWER.resetreas().write_value(pac::power::regs::Resetreas(raw));
        // SAFETY: written once in __pre_init, read-only afterwards.
        let marker = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(crate::hfxo::PREV_MARKER).cast::<u8>()) };
        pac::POWER
            .gpregret2()
            .write_value(pac::power::regs::Gpregret2(ALIVE_MARKER as u32));
        let reason = if raw & (1 << 3) != 0 {
            Reason::Lockup
        } else if raw & (1 << 1) != 0 {
            Reason::Watchdog
        } else if marker == ALIVE_MARKER {
            Reason::SoftReboot(0)
        } else if marker & 0xF0 == 0xA0 {
            Reason::SoftReboot(marker & 0x0F)
        } else if raw & (1 << 2) != 0 {
            Reason::SoftwareReset
        } else {
            Reason::Clean
        };

        let mut d = Self {
            led,
            linked: false,
            reason,
            ticks: 0,
            status_until: REASON_TICKS + STATUS_TICKS,
            notice: None,
        };
        d.apply();
        d
    }

    fn apply(&mut self) {
        if self.ticks < REASON_TICKS {
            match self.reason {
                Reason::Clean => self.led.set(false, false, false),
                Reason::Watchdog => self.led.set(false, false, true),
                Reason::SoftwareReset => self.led.set(true, false, true),
                Reason::Lockup => self.led.set(true, true, true),
                Reason::SoftReboot(0) => self.led.set(true, false, true),
                Reason::SoftReboot(code) => {
                    // `code` blinks of 500 ms on / 500 ms off, then dark.
                    let blink = self.ticks / 4;
                    let on = blink < code as u32 && self.ticks % 4 < 2;
                    self.led.set(on, false, on);
                }
            }
            return;
        }
        if let Some(notice) = self.notice {
            if self.ticks < notice.end() {
                let t = self.ticks - notice.start;
                let on = t < 2 * notice.blinks as u32 && t % 2 == 0;
                let (r, g, b) = notice.colour;
                self.led.set(on && r, on && g, on && b);
                return;
            }
            self.notice = None;
        }
        if self.ticks >= self.status_until {
            self.led.set(false, false, false);
            return;
        }
        if self.linked {
            self.led.set(false, true, false);
        } else {
            self.led.set(true, false, false);
        }
    }

    fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
        self.apply();
    }

    /// Announce a host BLE state: blink the profile number in the state's
    /// colour, after the reset reason has had its turn.
    fn notify_ble(&mut self, ble: BleStatus) {
        let colour = match ble.state {
            BleState::Connected => (false, false, true),
            BleState::Advertising => (true, true, false),
            BleState::Inactive => return,
        };
        self.notice = Some(Notice {
            colour,
            blinks: ble.profile.saturating_add(1),
            start: self.ticks.max(REASON_TICKS),
        });
        self.apply();
    }

    fn set_linked(&mut self, linked: bool) {
        self.linked = linked;
        // Light the new status for a while, but not before the reset reason
        // has had its turn.
        self.status_until = self.ticks.max(REASON_TICKS) + STATUS_TICKS;
        self.apply();
    }
}

// Each binary registers one of the two processors below; the other is dead
// code in that build.

/// Right half (central): the link is the peripheral's connection.
#[allow(dead_code)]
#[processor(subscribe = [PeripheralConnectedEvent, ConnectionStatusChangeEvent], poll_interval = 250)]
pub struct CentralStatusLed {
    display: Display,
    /// The last host BLE status seen, so only real changes are announced
    /// (the event fires for USB transitions too).
    ble: Option<BleStatus>,
}

#[allow(dead_code)]
impl CentralStatusLed {
    pub fn new(led: XiaoRgb) -> Self {
        Self {
            display: Display::new(led),
            ble: None,
        }
    }

    async fn poll(&mut self) {
        self.display.tick();
    }

    async fn on_peripheral_connected_event(&mut self, event: PeripheralConnectedEvent) {
        // One peripheral (id 0) in this split.
        self.display.set_linked(event.connected);
    }

    async fn on_connection_status_change_event(&mut self, event: ConnectionStatusChangeEvent) {
        let ble = event.0.ble;
        if self.ble == Some(ble) {
            return;
        }
        self.ble = Some(ble);
        self.display.notify_ble(ble);
    }
}

/// Left half (peripheral): the link is the central's connection.
#[allow(dead_code)]
#[processor(subscribe = [CentralConnectedEvent], poll_interval = 250)]
pub struct PeripheralStatusLed {
    display: Display,
}

#[allow(dead_code)]
impl PeripheralStatusLed {
    pub fn new(led: XiaoRgb) -> Self {
        Self { display: Display::new(led) }
    }

    async fn poll(&mut self) {
        self.display.tick();
    }

    async fn on_central_connected_event(&mut self, event: CentralConnectedEvent) {
        self.display.set_linked(event.connected);
    }
}
