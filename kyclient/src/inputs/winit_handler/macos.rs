// Project Kyber: macos.rs
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
use winit::dpi::{LogicalPosition, PhysicalPosition};

use kynput_rs::{InputType, MouseButtonType, MouseWheel};

use super::WinitEventType;

pub struct Impl {}

impl super::PlatformImpl for Impl {
    fn handled_events() -> HashMap<InputType, WinitEventType> {
        HashMap::from([
            (InputType::Keyboard, WinitEventType::Window),
            (InputType::MouseButton, WinitEventType::Window),
            (InputType::MouseMove, WinitEventType::Device),
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

    fn convert_window_mouse_button(button: &winit::event::MouseButton) -> Option<MouseButtonType> {
        match button {
            winit::event::MouseButton::Left => Some(MouseButtonType::Left),
            winit::event::MouseButton::Right => Some(MouseButtonType::Right),
            winit::event::MouseButton::Middle => Some(MouseButtonType::Middle),
            winit::event::MouseButton::Other(3) => Some(MouseButtonType::Side),
            winit::event::MouseButton::Other(4) => Some(MouseButtonType::Extra),
            _ => None,
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
