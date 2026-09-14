#![no_std]
#![no_main]

mod hfxo;
mod status_led;

use rmk::macros::rmk_peripheral;

/// Left half: split peripheral id 0, unified-matrix cols 0-4.
#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    /// Reset reason, then link status, on the Xiao's RGB LED.
    #[register_processor(poll)]
    fn status_led() -> crate::status_led::PeripheralStatusLed {
        crate::status_led::PeripheralStatusLed::new(crate::status_led::XiaoRgb::new(
            p.P0_26.into(),
            p.P0_30.into(),
            p.P0_06.into(),
        ))
    }
}
