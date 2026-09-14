//! Status on the Xiao's RGB LED: the cause of the last reset, the host BLE
//! link, and the split link.
//!
//! The colours follow ZMK's rgbled-widget, which this keyboard ran before,
//! and the pattern says what the colour alone could not: whether "not
//! connected" means a known host that has not answered yet, or a profile
//! with no host at all.
//!
//! # Host (right half only)
//!
//! Shown after every profile switch, and whenever the host link changes:
//!
//! | LED | host BLE |
//! | --- | --- |
//! | blue, solid 2 s | connected |
//! | red, slow breathing | the profile has a bond; waiting for that host (up to 30 s) |
//! | yellow, fast blink | the profile has no bond; open for pairing (up to 30 s) |
//! | off | inactive: USB mode, sleep, or nothing to say |
//!
//! # Split link (both halves)
//!
//! Green for 2 s when the other half connects, red for 2 s when it drops.
//! Solid, so it never reads as the host's breathing red.
//!
//! # Battery (both halves, their own cell)
//!
//! On the first reading after boot, 2 s of green (40 % and up), yellow
//! (20-39 %) or red (below 20 %) -- rgbled-widget's thresholds. After that
//! the LED stays quiet about the battery unless the level drops below 10 %,
//! when each further drop gets a short red blink.
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
//! | magenta, 5 blinks | the previous run **panicked**; the message is on the log of a `usb_log` build (`panic-record/`) |
//! | white | **CPU lockup** -- a fault inside a fault |
//!
//! # Power
//!
//! The halves run on LiPos, and a lit LED is a few milliamps -- as much as
//! the rest of the idle keyboard together -- so every pattern ends, and the
//! PWM peripheral is switched off between them (running, it keeps the 16 MHz
//! clock busy on its own). The three channels sit on PWM0, which nothing
//! else in this firmware uses.
//!
//! # Events
//!
//! The right half learns about the split link from
//! `PeripheralConnectedEvent` and the host from `ConnectionStatusChangeEvent`;
//! the left half from `CentralConnectedEvent`; both read their own cell from
//! `BatteryStatusEvent`. The connected-event channels are generated with one
//! subscriber slot and nothing else in this build takes them;
//! `connection_status_change` and `battery_status` have their base counts
//! raised in `keyboard.toml` for these subscribers. Watch that budget before adding a
//! subscription: overrunning it panics at startup, which on this firmware
//! means a board that never enumerates.

use embassy_nrf::Peri;
use embassy_nrf::gpio::{AnyPin, Level};
use embassy_nrf::pac;
use embassy_nrf::pwm::{DutyCycle, Instance, SimpleConfig, SimplePwm};
use rmk::event::{BatteryStatusEvent, CentralConnectedEvent, ConnectionStatusChangeEvent, PeripheralConnectedEvent};
use rmk::macros::processor;
use rmk::types::battery::BatteryStatus;
use rmk::types::ble::{BleState, BleStatus};

/// Tick period. Fine enough for the breathing ramp; the processor idles
/// otherwise.
const POLL_MS: u32 = 40;
const fn ticks(ms: u32) -> u32 {
    ms / POLL_MS
}

/// How long to hold the reset-reason display before anything else.
const REASON_TICKS: u32 = ticks(3000);
/// Solid indications (connected, split link).
const SOLID_TICKS: u32 = ticks(2000);
/// Waiting patterns (breathing, fast blink) give up after this, whether or
/// not the host turned up; the advertising itself carries on.
const WAIT_TICKS: u32 = ticks(30_000);
/// Fast blink: 100 ms on, 100 ms off.
const FAST_BLINK_TICKS: u32 = ticks(100);
/// Reset-reason code blinks: 500 ms on, 500 ms off.
const CODE_BLINK_TICKS: u32 = ticks(500);
/// Breathing period.
const BREATHE_TICKS: u32 = ticks(2000);
/// Battery bands, in percent: green at or above HIGH, yellow at or above LOW,
/// red below; a drop below CRITICAL blinks.
const BATTERY_HIGH: u8 = 40;
const BATTERY_LOW: u8 = 20;
const BATTERY_CRITICAL: u8 = 10;

/// PWM resolution: 1 MHz / 1000 = 1 kHz, above flicker.
const MAX_DUTY: u16 = 1000;

type Rgb = (u16, u16, u16);
const OFF: Rgb = (0, 0, 0);
const RED: Rgb = (MAX_DUTY, 0, 0);
const GREEN: Rgb = (0, MAX_DUTY, 0);
const BLUE: Rgb = (0, 0, MAX_DUTY);
const YELLOW: Rgb = (MAX_DUTY, MAX_DUTY / 2, 0);
const MAGENTA: Rgb = (MAX_DUTY, 0, MAX_DUTY);
const WHITE: Rgb = (MAX_DUTY, MAX_DUTY, MAX_DUTY);

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Solid,
    /// On for `half` ticks, off for `half` ticks.
    Blink { half: u32 },
    Breathe,
}

/// One thing the LED is saying, from `start` until `until`.
#[derive(Clone, Copy)]
struct Pattern {
    rgb: Rgb,
    kind: Kind,
    start: u32,
    until: u32,
}

impl Pattern {
    fn level(&self, tick: u32) -> Rgb {
        let t = tick.wrapping_sub(self.start);
        let scale = match self.kind {
            Kind::Solid => MAX_DUTY,
            Kind::Blink { half } => {
                if (t / half) % 2 == 0 {
                    MAX_DUTY
                } else {
                    0
                }
            }
            Kind::Breathe => {
                // Triangle wave, squared: the eye sees brightness roughly
                // logarithmically, so a linear ramp looks like a flash and a
                // long plateau. Never fully dark, so the rhythm reads as
                // breathing rather than blinking.
                let phase = t % BREATHE_TICKS;
                let half = BREATHE_TICKS / 2;
                let tri = if phase < half { phase } else { BREATHE_TICKS - phase };
                let lin = tri * MAX_DUTY as u32 / half;
                let sq = lin * lin / MAX_DUTY as u32;
                (sq as u16).max(MAX_DUTY / 40)
            }
        };
        (
            (self.rgb.0 as u32 * scale as u32 / MAX_DUTY as u32) as u16,
            (self.rgb.1 as u32 * scale as u32 / MAX_DUTY as u32) as u16,
            (self.rgb.2 as u32 * scale as u32 / MAX_DUTY as u32) as u16,
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Reason {
    Clean,
    Watchdog,
    SoftwareReset,
    Lockup,
    /// The previous run's GPREGRET2 marker survived: 0 = it was up and ended
    /// without recording why, 1..=4 = RMK's own reboot reason.
    SoftReboot(u8),
    /// The previous run panicked (`panic-record/`).
    Panic,
}

/// Written to GPREGRET2 once this firmware is up; RMK overwrites it with
/// `0xA0 | reason` if it reboots itself.
const ALIVE_MARKER: u8 = 0xA5;

/// The Xiao BLE's RGB LED: red P0.26, green P0.30, blue P0.06, common anode
/// (driving a pin low lights that colour), on three PWM channels.
pub struct XiaoRgb {
    pwm: SimplePwm<'static>,
    lit: bool,
}

impl XiaoRgb {
    pub fn new<T: Instance>(
        pwm: Peri<'static, T>,
        r: Peri<'static, AnyPin>,
        g: Peri<'static, AnyPin>,
        b: Peri<'static, AnyPin>,
    ) -> Self {
        let mut config = SimpleConfig::default();
        config.max_duty = MAX_DUTY;
        // High = off on a common-anode LED, and what the pins rest at while
        // the PWM is disabled.
        config.ch0_idle_level = Level::High;
        config.ch1_idle_level = Level::High;
        config.ch2_idle_level = Level::High;
        let pwm = SimplePwm::new_3ch(pwm, r, g, b, &config);
        pwm.disable();
        Self { pwm, lit: false }
    }

    fn set(&mut self, rgb: Rgb) {
        if rgb == OFF {
            if self.lit {
                // Park the pins high, then stop the counter: a disabled PWM
                // leaves the pins at their GPIO level.
                self.pwm.set_all_duties([DutyCycle::normal(0); 4]);
                self.pwm.disable();
                self.lit = false;
            }
            return;
        }
        if !self.lit {
            self.pwm.enable();
            self.lit = true;
        }
        // `normal(v)`: the pin is low for `v` of every MAX_DUTY ticks, which
        // on a common-anode LED is `v / MAX_DUTY` brightness.
        self.pwm.set_all_duties([
            DutyCycle::normal(rgb.0),
            DutyCycle::normal(rgb.1),
            DutyCycle::normal(rgb.2),
            DutyCycle::normal(0),
        ]);
    }
}

/// The display logic shared by both halves: four slots by priority, each
/// holding at most one pattern.
struct Display {
    led: XiaoRgb,
    tick: u32,
    boot: Option<Pattern>,
    battery: Option<Pattern>,
    host: Option<Pattern>,
    link: Option<Pattern>,
    /// The last battery level seen, so only the first reading and later
    /// critical drops light the LED.
    battery_level: Option<u8>,
    /// What the previous run said when it panicked, to repeat on the log
    /// until someone has had a chance to open the port.
    #[cfg(feature = "usb_log")]
    last_panic: Option<heapless::String<240>>,
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
        } else if marker == 0xA0 | panic_probe::PANIC_CODE {
            Reason::Panic
        } else if marker & 0xF0 == 0xA0 {
            Reason::SoftReboot(marker & 0x0F)
        } else if raw & (1 << 2) != 0 {
            Reason::SoftwareReset
        } else {
            Reason::Clean
        };
        let boot = match reason {
            Reason::Clean => None,
            Reason::Watchdog => Some((BLUE, Kind::Solid, REASON_TICKS)),
            Reason::Lockup => Some((WHITE, Kind::Solid, REASON_TICKS)),
            Reason::SoftwareReset | Reason::SoftReboot(0) => Some((MAGENTA, Kind::Solid, REASON_TICKS)),
            Reason::SoftReboot(code) => Some((
                MAGENTA,
                Kind::Blink { half: CODE_BLINK_TICKS },
                2 * CODE_BLINK_TICKS * code as u32,
            )),
            Reason::Panic => Some((MAGENTA, Kind::Blink { half: CODE_BLINK_TICKS }, 2 * CODE_BLINK_TICKS * 5)),
        }
        .map(|(rgb, kind, len)| Pattern {
            rgb,
            kind,
            start: 0,
            until: len,
        });

        let mut d = Self {
            led,
            tick: 0,
            boot,
            battery: None,
            host: None,
            link: None,
            battery_level: None,
            #[cfg(feature = "usb_log")]
            last_panic: panic_probe::take(),
        };
        #[cfg(not(feature = "usb_log"))]
        let _ = panic_probe::take();
        d.apply();
        d
    }

    /// The moment a new pattern may start: now, but never inside the
    /// reset-reason display.
    fn next_start(&self) -> u32 {
        self.tick.max(REASON_TICKS)
    }

    fn show(slot: &mut Option<Pattern>, start: u32, rgb: Rgb, kind: Kind, len: u32) {
        *slot = Some(Pattern {
            rgb,
            kind,
            start,
            until: start + len,
        });
    }

    fn apply(&mut self) {
        let tick = self.tick;
        for slot in [&mut self.boot, &mut self.battery, &mut self.host, &mut self.link] {
            if let Some(p) = *slot {
                if tick >= p.until {
                    *slot = None;
                    continue;
                }
                if tick >= p.start {
                    let rgb = p.level(tick);
                    self.led.set(rgb);
                    return;
                }
            }
        }
        self.led.set(OFF);
    }

    fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        // The log is a small pipe that nobody may be reading yet; say it
        // every 5 s for the first minute, then let it go.
        #[cfg(feature = "usb_log")]
        if let Some(text) = &self.last_panic
            && self.tick % ticks(5000) == 1
        {
            ::log::error!("previous run panicked: {}", text.as_str());
            if self.tick > ticks(60_000) {
                self.last_panic = None;
            }
        }
        self.apply();
    }

    /// Host BLE status (right half): what a profile switch or a link change
    /// means for the user, in rgbled-widget's colours.
    fn host_status(&mut self, ble: BleStatus) {
        let start = self.next_start();
        match ble.state {
            BleState::Connected => Self::show(&mut self.host, start, BLUE, Kind::Solid, SOLID_TICKS),
            BleState::Advertising if rmk::ble::is_profile_bonded(ble.profile) => {
                Self::show(&mut self.host, start, RED, Kind::Breathe, WAIT_TICKS)
            }
            BleState::Advertising => Self::show(
                &mut self.host,
                start,
                YELLOW,
                Kind::Blink { half: FAST_BLINK_TICKS },
                WAIT_TICKS,
            ),
            BleState::Inactive => self.host = None,
        }
        self.apply();
    }

    /// Own battery: the first reading after boot in rgbled-widget's bands,
    /// then only critical drops.
    fn battery_status(&mut self, status: BatteryStatus) {
        let BatteryStatus::Available { level: Some(level), .. } = status else {
            return;
        };
        let start = self.next_start();
        match self.battery_level {
            None => {
                let rgb = if level >= BATTERY_HIGH {
                    GREEN
                } else if level >= BATTERY_LOW {
                    YELLOW
                } else {
                    RED
                };
                Self::show(&mut self.battery, start, rgb, Kind::Solid, SOLID_TICKS);
            }
            Some(prev) if level < BATTERY_CRITICAL && level < prev => {
                Self::show(&mut self.battery, start, RED, Kind::Blink { half: FAST_BLINK_TICKS }, ticks(600));
            }
            _ => {}
        }
        self.battery_level = Some(level);
        self.apply();
    }

    /// The split link came or went.
    fn link_status(&mut self, linked: bool) {
        let start = self.next_start();
        let rgb = if linked { GREEN } else { RED };
        Self::show(&mut self.link, start, rgb, Kind::Solid, SOLID_TICKS);
        self.apply();
    }
}

// Each binary registers one of the two processors below; the other is dead
// code in that build.

/// Right half (central): the split link is the peripheral's connection, and
/// the host link is its own.
#[allow(dead_code)]
#[processor(subscribe = [PeripheralConnectedEvent, ConnectionStatusChangeEvent, BatteryStatusEvent], poll_interval = 40)]
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
        self.display.step();
    }

    async fn on_peripheral_connected_event(&mut self, event: PeripheralConnectedEvent) {
        // One peripheral (id 0) in this split.
        self.display.link_status(event.connected);
    }

    async fn on_connection_status_change_event(&mut self, event: ConnectionStatusChangeEvent) {
        let ble = event.0.ble;
        if self.ble == Some(ble) {
            return;
        }
        self.ble = Some(ble);
        self.display.host_status(ble);
    }

    async fn on_battery_status_event(&mut self, event: BatteryStatusEvent) {
        self.display.battery_status(event.0);
    }
}

/// Left half (peripheral): the link is the central's connection.
#[allow(dead_code)]
#[processor(subscribe = [CentralConnectedEvent, BatteryStatusEvent], poll_interval = 40)]
pub struct PeripheralStatusLed {
    display: Display,
}

#[allow(dead_code)]
impl PeripheralStatusLed {
    pub fn new(led: XiaoRgb) -> Self {
        Self { display: Display::new(led) }
    }

    async fn poll(&mut self) {
        self.display.step();
    }

    async fn on_central_connected_event(&mut self, event: CentralConnectedEvent) {
        self.display.link_status(event.connected);
    }

    async fn on_battery_status_event(&mut self, event: BatteryStatusEvent) {
        self.display.battery_status(event.0);
    }
}
