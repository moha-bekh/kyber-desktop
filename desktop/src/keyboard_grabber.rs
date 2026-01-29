// Project Kyber: keyboard_grabber.rs
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

use std::sync::Arc;

use log::{error, info};
use winit::{
    event_loop::{DeviceEventFilter, DeviceEventFilterTarget, EventLoopWindowTarget},
    window::Window,
};

use crate::inputs::WinitHandler;
use crate::UserEvent;

// # How to create an immersive client experience

// ## Presentation

// A cool feature for users is to disable the local Alt+Tab shortcut, but let it be
// forwarded to the Host OS. This gives an immersive experience. A more OS specific
// example is to disable the local Windows key on Windows clients.

// However, the implementation depends on the OS, so many strategy must be implemented.

// ## X11

// XGrabKeyboard()/XUngrabKeyboard() can directly disable the local Alt+Tab shortcut.
// This call can be done directly by the code that handles the window interaction.

// Furthermore, those calls have no impact on the way to acquire the inputs.

// ## Windows

// ### WH_KEYBOARD_LL

// There are no direct way to disable some shortcuts on Windows.
// The only way is to use SetWindowsHookEx(WH_KEYBOARD_LL): this API provides a way
// to hook the input delivery chain and to drop events. MSDN warns this API is
// dangerous if incorrectly used because it has a system-wide impact, so a hook
// must be unregistered when not required.

// When this hook drops key press or release, the event isn't propagated in the
// system. Because of this, WM_KEYDOWN/WM_KEYUP won't be received in WindowProc().
// So when a hook is plugged, it is required to only use the key press/release it
// receives.

// SetWindowsHookEx(WH_KEYBOARD_LL) has a requirement: the callback is called
// on the same thread that has registered the hook. Furthermore, this thread must
// have a message loop.

// ### RawInput

// A WindowProc callback can acquire inputs with WM_KEYDOWN/WM_KEYUP events, but
// there is another way to acquire inputs: RegisterRawInputDevices().

// This method is useful to get the information sent by mouses and keyboards
// without any extra processing that Windows could add. It provides the same level
// of low level access than WH_KEYBOARD_LL, but it can't disable keyboard events.

// It seems that the usage of RegisterRawInputDevices() and
// SetWindowsHookEx(WH_KEYBOARD_LL) are mutually exclusive.

// ### Strategy

// To reduce the impact of LowLevelKeyboard, it must be used only
// when we want to enable an immersive experience for the user, for example when
// the application is fullscreen. The hook **must** be disabled when the
// application has lost focus.

// The solution is to use RawInput and switch to LowLevelKeyboard only when
// immersive mode is required.

// ```
// -------------------------------------------------
// |                 App event loop                |
// -------------------------------------------------
//             ^                      |
//             |                      ⌄
//             |          --------------------------
//             |          |      InputHandler      | ----------------
//             |          --------------------------                |
//             |                                                    |
//             |                                                    |
//             |          --------------------------                |
//             |          |      LowLevelHook      | -------------- |
//             |          --------------------------              | |
//             |                      ^                           | |
//             |                      |                           | |
// ------------------------           |                           | |
// |        winit         |           |                           | |
// ------------------------           |                           | |
//             ^                      |                           | |
//             |                      |                           ⌄ ⌄
// -------------------------------------------------         -------------
// | WindowProc(WM_INPUT) | LowLevelKeyboardProc() |         | libkynput |
// -------------------------------------------------         -------------
//             ^                     ^
//             |                     |
// -------------------------------------------------
// |       RawInput       |    LowLevelKeyboard    |
// -------------------------------------------------
// ```

// The default keyboard acquisition mode let winit get RawInput (exposed as
// winit::event::DeviceEvent). The application event loop gets the events and
// forward them to InputHandler. This layer acts as a filter between the
// application event loop and libkynput. It forwards only the events that are not
// automatically acquired directly by libkynput.

// To enable the immersive mode on windows, the app event loop must request winit
// to unplug RawInput, and then plug a LowLevelKeyboard hook. In this case:
// * The application event loop doesn't receive RawInput anymore
// * The LowLevelKeyboard hook produces all the keyboard events

// The LowLevelHook component is actually a special libkynput producer than can be
// instantiated and deleted by the application during runtime. The switch only
// change the source of the keyboard events.

pub struct KeyboardGrabber {
    has_custom_grabber: bool,
    input_handler: Option<Arc<WinitHandler>>,
    is_grabbing: bool,
}

impl KeyboardGrabber {
    pub fn new() -> Self {
        Self {
            has_custom_grabber: WinitHandler::has_custom_keyboard_grabber(),
            input_handler: None,
            is_grabbing: false,
        }
    }

    pub fn set_input_handler(&mut self, input_handler: Arc<WinitHandler>) {
        self.input_handler = Some(input_handler);
    }

    pub fn grab(
        &mut self,
        event_loop: &EventLoopWindowTarget<UserEvent>,
        window: &Window,
        grab: bool,
    ) {
        if self.is_grabbing == grab {
            return;
        }

        if self.has_custom_grabber {
            let Some(input_handler) = self.input_handler.as_mut() else {
                return;
            };

            // Call order should make sure that the same event won't be raised
            // by two input sources.
            if grab {
                event_loop.set_device_event_filter(
                    DeviceEventFilter::default(),
                    DeviceEventFilterTarget::all() & !DeviceEventFilterTarget::KEYBOARD,
                );

                input_handler.plug_keyboard_grabber(true);
            } else {
                input_handler.plug_keyboard_grabber(false);

                event_loop.set_device_event_filter(
                    DeviceEventFilter::default(),
                    DeviceEventFilterTarget::all(),
                );
            }
        } else {
            let ret = window.grab_keyboard(grab);
            if let Err(err) = ret {
                error!("Failed to grab keyboard: {err:?}");
                return;
            }
        }

        info!("Keyboard grab: {}", grab);

        self.is_grabbing = grab;
    }
}
