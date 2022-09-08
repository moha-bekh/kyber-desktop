use std::collections::HashMap;

use keycode::{KeyMap, KeyMapping};

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
}
