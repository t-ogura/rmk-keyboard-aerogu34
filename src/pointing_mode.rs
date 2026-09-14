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

//! # Vial and Rynk flavours
//!
//! With Vial there is no host-side pointing configuration, so this
//! controller drives the mode from the layer, as above. With Rynk the
//! firmware carries a runtime pointing configuration -- per-device default
//! mode plus per-layer overrides -- that the host (rynkbench) reads,
//! edits and stores, and the processor follows it by itself. Publishing a
//! mode from here would pin the processor to "explicit" mode and shut the
//! host out, so under `rynk` this controller only **seeds** that
//! configuration on a board that has none yet (first boot after flashing),
//! with the same values the Vial flavour hard-codes, and then stays quiet.

#[cfg(not(feature = "rynk"))]
use rmk::event::{PointingProcessorEvent, publish_event};
use rmk::event::LayerChangeEvent;
use rmk::input_device::pointing::{CursorConfig, PointingMode, ScrollConfig};
use rmk::macros::processor;

/// The trackball's device id, matching `[[split.central.input_device.paw3222]]`.
const TRACKBALL_ID: u8 = 0;

/// ZMK's Scroll layer, where the ball drives the wheel instead of the cursor.
const SCROLL_LAYER: u8 = 3;

/// The auto mouse layer. Raised by trackball motion rather than by the user,
/// so it carries no opinion about which mode the trackball should be in.
#[cfg(not(feature = "rynk"))]
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
    #[cfg(not(feature = "rynk"))]
    mode: PointingMode,
    /// Rynk: whether the runtime configuration has been seeded (or found).
    #[cfg(feature = "rynk")]
    seeded: bool,
}

impl PointingModeController {
    pub fn new() -> Self {
        Self {
            #[cfg(not(feature = "rynk"))]
            mode: CURSOR_MODE,
            #[cfg(feature = "rynk")]
            seeded: false,
        }
    }

    #[cfg(not(feature = "rynk"))]
    fn publish(&self) {
        publish_event(PointingProcessorEvent {
            device_id: TRACKBALL_ID,
            mode: self.mode,
        });
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        #[cfg(feature = "rynk")]
        {
            let _ = event;
        }
        #[cfg(not(feature = "rynk"))]
        {
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
    }

    async fn poll(&mut self) {
        #[cfg(feature = "rynk")]
        {
            if !self.seeded {
                self.seeded = seed_runtime_config().await;
            }
        }
        #[cfg(not(feature = "rynk"))]
        self.publish();
    }
}

/// Rynk: give a never-configured board the Vial flavour's policy -- cursor
/// by default, the wheel on the scroll layer -- so the trackball works
/// before anyone opens the GUI. A board that already holds a configuration
/// is left alone: that is the user's, edited from the host.
///
/// Returns whether the configuration is settled (found or written); the
/// caller retries on the next poll otherwise, e.g. while storage is still
/// restoring it.
#[cfg(feature = "rynk")]
async fn seed_runtime_config() -> bool {
    use rmk::input_device::pointing_config;
    use rmk::types::protocol::rynk::{PointingConfig, PointingDeviceConfig, PointingLayerOverride};

    let current = pointing_config::get().await;
    if current.device_count > 0 {
        return true;
    }
    let mut config = PointingConfig::default();
    config.revision = current.revision;
    config.device_count = 1;
    config.devices[0] = PointingDeviceConfig {
        device_id: TRACKBALL_ID,
        mode: CURSOR_MODE,
    };
    config.override_count = 1;
    config.overrides[0] = PointingLayerOverride {
        layer: SCROLL_LAYER,
        device_id: TRACKBALL_ID,
        mode: SCROLL_MODE,
    };
    // `replace` validates against the keymap's layer count; NUM_LAYER is not
    // reachable from here, so pass one that admits the scroll layer.
    pointing_config::replace(config, SCROLL_LAYER as usize + 1).await.is_ok()
}
