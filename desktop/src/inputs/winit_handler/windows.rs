use std::collections::HashMap;

use libkynput_rs::{Consumer, InputType, Modifiers, MouseButtonType, MouseWheel};
use log::error;

use super::WinitEventType;
use crate::inputs::winit_handler::{scancodes::*, KeyboardGrabber};

// Implement the KeyboardGrabber trait for libkynput's KeyboardGrabber
impl<F> KeyboardGrabber for libkynput_rs::windows::KeyboardGrabber<F> where
    F: Fn(u16, &Modifiers) -> bool + Unpin
{
}

pub struct Impl {}

/// Function called to filter scancodes.
///
/// Returns true if the scancode must be filtered, else false
fn filter_keyboard_cb(scancode: u16, _modifiers: &Modifiers) -> bool {
    // Because the leds on the keyboard are managed by Windows, we don't want to filter
    // the keys that can change the state of the leds. If we filter them, the leds
    // will not change state when the user press them.
    let led_related_keys = (scancode == SCANCODE_SCROLLLOCK)
        || (scancode == SCANCODE_NUMLOCK)
        || (scancode == SCANCODE_CAPTSLOCK);

    !led_related_keys
}

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

    fn has_custom_keyboard_grabber() -> bool {
        true
    }

    fn create_keyboard_grabber(consumer: Consumer) -> Option<Box<dyn super::KeyboardGrabber>> {
        let ret = libkynput_rs::windows::KeyboardGrabber::new(consumer, filter_keyboard_cb);
        match ret {
            Ok(grabber) => Some(Box::new(grabber)),
            Err(err) => {
                error!("Failed to create KeyboardGrabber: {err:?}");
                None
            }
        }
    }
}
