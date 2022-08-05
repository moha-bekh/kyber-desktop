use std::collections::HashMap;

use keycode::{KeyMap, KeyMapping};

use libkynput_rs::{InputType, MouseWheel};

use super::WinitEventType;

// Linux keycode => Scancode conversion. Check KeyboardKey's
// documentation for more details.
fn keycode_to_scancode(keycode: u16) -> Option<u16> {
    if let Ok(keymap) = KeyMap::from_key_mapping(KeyMapping::Evdev(keycode)) {
        Some(keymap.win)
    } else {
        None
    }
}

pub struct Impl {}

impl super::PlatformImpl for Impl {
    fn handled_events() -> HashMap<InputType, WinitEventType> {
        HashMap::from([
            (InputType::Keyboard, WinitEventType::Device),
            (InputType::MouseButton, WinitEventType::Device),
            (InputType::MouseWheel, WinitEventType::Device),
            (InputType::MouseMove, WinitEventType::Device),
        ])
    }

    fn convert_scancode(scancode: u16) -> Option<u16> {
        keycode_to_scancode(scancode)
    }

    fn convert_mouse_wheel(horizontal: f32, vertical: f32) -> MouseWheel {
        // winit forwards two kind of value range:
        // - [-1.0;1.0]: Trackpad values
        // - [-15;15]: Mouse wheel values
        if horizontal.abs() >= 15.0 || vertical.abs() >= 15.0 {
            // Normalize mouse wheel values
            MouseWheel {
                dx: horizontal / 15.0,
                dy: vertical / 15.0,
            }
        } else {
            // winit values seem to be too high for trackpad values. Reduce by a 10 factor seems to
            // give a correct result for now. The same issue has been handled in libinput
            // implementation.
            MouseWheel {
                dx: horizontal / 10.0,
                dy: vertical / 10.0,
            }
        }
    }
}
