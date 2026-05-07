// Project Kyber: linux.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0-or-later
//
// This file is both under dual license: AGPLv3 and a Commercial one.
//
// ----
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::HashMap;

use keycode::{KeyMap, KeyMapping};

use kynput_rs::{InputType, MouseButtonType, MouseWheel};

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

    fn convert_device_mouse_button(button: u32) -> Option<MouseButtonType> {
        match button {
            1 => Some(MouseButtonType::Left),
            2 => Some(MouseButtonType::Middle),
            3 => Some(MouseButtonType::Right),
            8 => Some(MouseButtonType::Side),
            9 => Some(MouseButtonType::Extra),
            _ => None,
        }
    }
}
