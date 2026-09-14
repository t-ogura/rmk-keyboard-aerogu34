//! Trackball behaviour per layer -- the RMK half of ZMK's input-processor
//! chain.
//!
//! The ZMK shield (aerogu34_right.overlay) expressed this on the trackball
//! listener:
//!
//! ```dts
//! input-processors = <&deadzone_processor                 // threshold 10, 300 ms
//!                     &zip_xy_transform (X_INVERT | Y_INVERT)
//!                     &mouse_runtime_input_processor      // DYA Studio CPI etc.
//!                     &zip_temp_layer 4 1000              // auto mouse layer
//!                     &scroll_runtime_input_processor>;   // XY -> wheel on layer 3
//! ```
//!
//! Most of that is configuration in RMK and lives in `keyboard.toml`: the
//! 180-degree rotation is `invert_x` / `invert_y` on the sensor, the CPI is
//! `cpi`, and the auto mouse layer is `[[behavior.auto_mouse_layer]]`. What
//! is left is the per-layer mode switch, which is this file. (The deadzone
//! and the runtime tuning have no RMK counterpart yet.)
//!
//! | layer | mode |
//! | --- | --- |
//! | 3 (scroll) | `Scroll`, 1/12 per axis |
//! | 4 (mouse) | whatever was already active |
//! | anything else | `Cursor`, 1:1 |
//!
//! # Living with `LayerChangeEvent`
//!
//! The event reports only the **top** active layer, which makes the auto mouse
//! layer (4, above the scroll layer) ambiguous: `LayerChangeEvent(4)` says
//! nothing about whether layer 3 is held underneath. Two measures keep the
//! mode honest anyway, and both matter:
//!
//! - **Layer 4 leaves the mode alone.** It is not a layer the user selects,
//!   it is one the trackball raises, so it should not mean "cursor".
//! - **`exclude_layers = [3]` in `keyboard.toml`.** The auto mouse layer never
//!   activates over the scroll layer, and steps aside immediately if it was
//!   already up, so a definitive `LayerChangeEvent(3)` always follows.

use rmk::event::{LayerChangeEvent, PointingProcessorEvent, publish_event};
use rmk::input_device::pointing::{CursorConfig, PointingMode, ScrollConfig};
use rmk::macros::processor;

/// The trackball's device id, matching `[[split.central.input_device.paw3222]]`.
const TRACKBALL_ID: u8 = 0;

/// ZMK's Scroll layer, where the ball drives the wheel instead of the cursor.
const SCROLL_LAYER: u8 = 3;

/// The auto mouse layer. Raised by trackball motion rather than by the user,
/// so it carries no opinion about which mode the trackball should be in.
const MOUSE_LAYER: u8 = 4;

/// 1:1, as the ZMK chain had no scaler. The 180-degree rotation is applied
/// once, by the sensor's `invert_x` / `invert_y`, before any mode sees the
/// sample.
const CURSOR_MODE: PointingMode = PointingMode::Cursor(CursorConfig {
    multiplier_x: 1,
    multiplier_y: 1,
    invert_x: false,
    invert_y: false,
});

/// The ZMK scroll processor mapped XY to the wheel 1:1 and was tuned at
/// runtime through DYA Studio; 1/12 is the Cornix TB's setting and a
/// reasonable start. The sensor accumulates the remainder between samples,
/// so slow movement still scrolls.
///
/// Both axes are inverted here. The sensor's own registers already flip X and
/// Y for the cursor (the 180-degree mounting), and RMK's scroll mode maps
/// sensor +Y to a *negative* wheel, so with the wheel the two flips stacked
/// into "ball down, page up" on the Cornix TB; `invert_y` undoes it.
const SCROLL_MODE: PointingMode = PointingMode::Scroll(ScrollConfig {
    multiplier_x: 1,
    divisor_x: 12,
    multiplier_y: 1,
    divisor_y: 12,
    invert_x: true,
    invert_y: true,
});

/// Re-announces the active mode every 200 ms, as well as on layer changes.
///
/// The mode is state held by another task, reached only by a published event,
/// and nothing acknowledges it. `PointingProcessor` multiplexes motion and
/// mode events with `select_biased!` -- motion first -- so a mode event can
/// wait behind a burst of samples. Repeating it on a timer turns a wrong mode
/// that persists into one that corrects itself. `set_pointing_mode` only
/// assigns, so repeating the current mode is free: in particular it does not
/// disturb the scroll accumulator.
#[processor(subscribe = [LayerChangeEvent], poll_interval = 200)]
pub struct PointingModeController {
    mode: PointingMode,
}

impl PointingModeController {
    pub fn new() -> Self {
        Self { mode: CURSOR_MODE }
    }

    fn publish(&self) {
        publish_event(PointingProcessorEvent {
            device_id: TRACKBALL_ID,
            mode: self.mode,
        });
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        let mode = match event.0 {
            SCROLL_LAYER => SCROLL_MODE,
            MOUSE_LAYER => return,
            _ => CURSOR_MODE,
        };
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        self.publish();
    }

    async fn poll(&mut self) {
        self.publish();
    }
}
