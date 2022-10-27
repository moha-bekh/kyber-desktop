use std::collections::HashMap;

use keycode::{KeyMap, KeyMapping};
use winit::dpi::{LogicalPosition, PhysicalPosition};

use libkynput_rs::{InputType, MouseWheel};

use super::WinitEventType;

pub struct Impl {}

impl super::PlatformImpl for Impl {
    fn handled_events() -> HashMap<InputType, WinitEventType> {
        HashMap::from([
            (InputType::Keyboard, WinitEventType::Window),
            (InputType::MouseButton, WinitEventType::Window),
            (InputType::MouseWheel, WinitEventType::Window),
        ])
    }

    fn convert_scancode(keycode: u16) -> Option<u16> {
        if let Ok(keymap) = KeyMap::from_key_mapping(KeyMapping::Mac(keycode)) {
            Some(keymap.win)
        } else {
            None
        }
    }

    fn convert_mouse_wheel(horizontal: f32, vertical: f32) -> MouseWheel {
        MouseWheel {
            dx: -horizontal,
            dy: -vertical,
        }
    }

    fn convert_mouse_wheel_pixel_delta(
        position: &PhysicalPosition<f64>,
        scale_factor: f64,
    ) -> Option<MouseWheel> {
        // Should slow down the values to a correct feeling.
        let scale = 16.0;

        // winit returns a position that depends on the scale factor. Revert it to get
        // a consistent behavior.
        let position: LogicalPosition<f32> = position.to_logical(scale_factor);

        // Horizontal scroll is currently disabled because its generates some issues on
        // Windows Hosts
        Some(MouseWheel {
            dx: 0.0,
            dy: -(position.y / scale),
        })
    }
}
