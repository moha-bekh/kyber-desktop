// Project Kyber: mod.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0
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

mod multi_mouse_drag;
mod multi_mouse_position_fullscreen;
mod multi_mouse_position_windowed;
mod multi_relative_fullscreen;
mod single_fullscreen_toggle;
mod single_keypress;
mod single_mouse_drag;
mod single_mouse_position_windowed;
mod single_relative;

use mockall::predicate::*;
use mockall::*;
use raw_window_handle::RawWindowHandle;

use kyclient_rs::Display;
use kynput_rs::{InputTarget, InputType};

use super::*;
use crate::event_loop::UserEvent;
use crate::window::Size;

// SAFETY: This creates a fake EventLoopWindowTarget for tests only.
// It's safe because we never actually use it - keyboard_grabber is None in tests.
fn fake_event_loop_target() -> &'static winit::event_loop::EventLoopWindowTarget<UserEvent> {
    unsafe { std::mem::transmute(&()) }
}

struct InputProducer {
    consumer: kynput_rs::Consumer,
}

impl InputProducer {
    fn new(consumer: kynput_rs::Consumer) -> Self {
        Self { consumer }
    }
}

impl kynput_rs::HandlerInterface for InputProducer {
    fn start(&self) {}

    fn stop(&self) {}

    fn produced_types(&self) -> Option<Vec<InputType>> {
        Some(vec![
            InputType::Keyboard,
            InputType::MouseMove,
            InputType::MouseButton,
        ])
    }
}

mock! {
    InputNetwork {

    }

    impl kynput_rs::ConsumerInterface for InputNetwork {
        fn consume(&self, packet: kynput_rs::Packet);
    }
}

mock! {
    EventLoopProxy {
    }

    impl crate::event_loop::EventProxy for EventLoopProxy {
        fn send_event(&self, event: UserEvent) -> Result<()>;
    }
}

#[derive(Clone)]
struct EventLoopProxy {
    proxy: Arc<MockEventLoopProxy>,
}

impl EventLoopProxy {
    fn new(proxy: MockEventLoopProxy) -> Self {
        Self {
            proxy: Arc::new(proxy),
        }
    }
}

impl crate::event_loop::EventProxy for EventLoopProxy {
    fn send_event(&self, event: UserEvent) -> Result<()> {
        self.proxy.send_event(event)
    }
}

mock! {
    WindowControl {
    }

    impl crate::window::WindowControl for WindowControl {
        fn id(&self) -> WindowId;

        fn set_cursor_visible(&self, visible: bool);

        fn set_cursor_position(&self, position: Position) -> Result<()>;

        fn set_cursor_rgba(&self, cursor: Cursor);

        fn monitor_size(&self) -> Option<Size>;

        fn inner_position(&self) -> Option<Position>;

        fn inner_size(&self) -> Size;

        fn is_fullscreen(&self) -> bool;

        fn set_fullscreen(&self, fullscreen: bool);

        fn raw_window_handle(&self) -> RawWindowHandle;

        fn scale_factor(&self) -> f64;

        fn set_cursor_rect(&self, rect: Option<(Position, Size)>) -> Result<()>;

        fn set_cursor_grab(&self, grab: bool) -> Result<()>;

        fn gain_focus(&self);

        fn winit_window(&self) -> &winit::window::Window;
    }
}
