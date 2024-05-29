use std::collections::HashMap;

use libkynput_rs::{InputType, MouseButtonType, MouseWheel};

use super::WinitEventType;

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
        Some(scancode)
    }

    fn convert_mouse_wheel(horizontal: f32, vertical: f32) -> MouseWheel {
        MouseWheel {
            dx: -horizontal,
            dy: -vertical,
        }
    }

    fn convert_device_mouse_button(button: u32) -> Option<MouseButtonType> {
        match button {
            1 => Some(MouseButtonType::Left),
            2 => Some(MouseButtonType::Middle),
            3 => Some(MouseButtonType::Right),
            4 => Some(MouseButtonType::Side),
            5 => Some(MouseButtonType::Extra),
            _ => None,
        }
    }
}
