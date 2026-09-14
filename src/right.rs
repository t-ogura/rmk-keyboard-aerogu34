#![no_std]
#![no_main]

mod hfxo;
mod pointing_mode;
mod status_led;

use rmk::macros::rmk_central;

/// Right half: split central. It owns the keymap, Vial, the trackball and the
/// USB/BLE link to the host; the left half's keys arrive over BLE.
#[rmk_central]
mod keyboard_central {
    /// Switch the trackball between cursor and scroll behaviour per layer.
    ///
    /// The sensor, its 180-degree mounting, its CPI and the auto mouse layer
    /// are all declared in `keyboard.toml`; only the per-layer mode switch
    /// needs code.
    #[register_processor(poll)]
    fn pointing_mode() -> crate::pointing_mode::PointingModeController {
        crate::pointing_mode::PointingModeController::new()
    }

    /// Reset reason, then split status, on the Xiao's RGB LED.
    ///
    /// The body is inlined into `main` after the peripherals are taken, so
    /// `p` is in scope here. `poll`, not `event`: `polling_loop()` selects
    /// over both the subscription and the tick; `process_loop()` only ever
    /// waits on events, so the LED would never advance past the reset colour.
    #[register_processor(poll)]
    fn status_led() -> crate::status_led::CentralStatusLed {
        crate::status_led::CentralStatusLed::new(crate::status_led::XiaoRgb::new(
            p.P0_26.into(),
            p.P0_30.into(),
            p.P0_06.into(),
        ))
    }
}
