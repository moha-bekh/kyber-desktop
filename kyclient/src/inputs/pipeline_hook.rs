// Project Kyber: pipeline_hook.rs
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

use std::collections::HashSet;
use std::sync::{Arc, LazyLock, Mutex};

use keycode::{KeyMap, KeyMappingId};
use log::warn;

use kynput_rs::{InputType, KeyboardKey, MouseButton, MouseButtonType, Packet};
use kynput_rs::{RouterHook, RouterHookPacketResult};

use crate::event_loop::{EventProxy, UserEvent};

const SCANCODE_UNKNOWN: u16 = 0;

static SHORTCUTS: LazyLock<Vec<(Vec<u16>, ShortcutType)>> = LazyLock::new(|| {
    let ctrl = KeyMap::from(KeyMappingId::ControlLeft).win;
    let alt = KeyMap::from(KeyMappingId::AltLeft).win;
    let b = KeyMap::from(KeyMappingId::UsB).win;
    vec![
        (
            vec![KeyMap::from(KeyMappingId::UsF).win, ctrl, alt],
            ShortcutType::Fullscreen,
        ),
        (
            vec![KeyMap::from(KeyMappingId::UsR).win, ctrl, alt],
            ShortcutType::RestartStreaming,
        ),
        (
            vec![KeyMap::from(KeyMappingId::UsM).win, ctrl],
            ShortcutType::ToggleMute,
        ),
        (
            vec![KeyMap::from(KeyMappingId::ArrowUp).win, ctrl],
            ShortcutType::VolumeUp,
        ),
        (
            vec![KeyMap::from(KeyMappingId::ArrowDown).win, ctrl],
            ShortcutType::VolumeDown,
        ),
        (
            vec![KeyMap::from(KeyMappingId::ArrowUp).win, ctrl, b],
            ShortcutType::BitrateUp,
        ),
        (
            vec![KeyMap::from(KeyMappingId::ArrowDown).win, ctrl, b],
            ShortcutType::BitrateDown,
        ),
    ]
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutType {
    Fullscreen,
    RestartStreaming,
    VolumeUp,
    VolumeDown,
    BitrateUp,
    BitrateDown,
    ToggleMute,
}

struct InternalState {
    scancodes: HashSet<u16>,
    mouse_btns: HashSet<MouseButtonType>,
}

impl InternalState {
    fn new() -> Self {
        Self {
            scancodes: HashSet::new(),
            mouse_btns: HashSet::new(),
        }
    }

    fn reset_keyboard_state(&mut self) {
        self.scancodes.clear();
    }

    fn handle_shortcut(&mut self, event: &KeyboardKey) -> Option<ShortcutType> {
        // Update keyboard state
        if event.scancode != SCANCODE_UNKNOWN {
            if event.pressed {
                self.scancodes.insert(event.scancode);
            } else {
                self.scancodes.remove(&event.scancode);
            }
        }

        // Find active shortcut
        SHORTCUTS.iter().find_map(|(scancodes, shortcut)| {
            if scancodes.len() != self.scancodes.len() {
                return None;
            }

            let shortcut_active = scancodes.iter().all(|s| self.scancodes.contains(s));
            if shortcut_active {
                Some(*shortcut)
            } else {
                None
            }
        })
    }

    fn handle_mouse_button(&mut self, btn: &MouseButton) {
        if btn.pressed {
            self.mouse_btns.insert(btn.button);
        } else {
            self.mouse_btns.remove(&btn.button);
        }
    }

    pub fn has_one_mouse_button_pressed(&self) -> bool {
        !self.mouse_btns.is_empty()
    }
}

pub struct PipelineStateReader {
    state: Arc<Mutex<InternalState>>,
}

impl PipelineStateReader {
    pub fn has_one_mouse_button_pressed(&self) -> bool {
        match self.state.lock() {
            Ok(state) => state.has_one_mouse_button_pressed(),
            Err(err) => {
                warn!("PipelineStateReader: Fail to lock state: {:?}", err);
                false
            }
        }
    }

    pub fn flush_keys(&self) -> bool {
        match self.state.lock() {
            Ok(mut state) => {
                state.reset_keyboard_state();
                true
            }
            Err(err) => {
                warn!("PipelineStateReader: Fail to lock state: {:?}", err);
                false
            }
        }
    }
}

pub struct ClientPipelineHook<T>
where
    T: EventProxy,
{
    event_proxy: T,
    state: Arc<Mutex<InternalState>>,
}

impl<T> ClientPipelineHook<T>
where
    T: EventProxy,
{
    pub fn new(event_proxy: T) -> Self {
        Self {
            event_proxy,
            state: Arc::new(Mutex::new(InternalState::new())),
        }
    }

    pub fn get_state_reader(&self) -> PipelineStateReader {
        PipelineStateReader {
            state: self.state.clone(),
        }
    }
}

impl<T> RouterHook for ClientPipelineHook<T>
where
    T: EventProxy,
{
    fn on_packet(&self, pkt: &Packet) -> RouterHookPacketResult {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(err) => {
                warn!("InputRouterHook::on_packet(): Lock failed: {:?}", err);
                return RouterHookPacketResult {
                    process_event: true,
                };
            }
        };

        match pkt.get_type() {
            InputType::Keyboard => {
                if let Some(key) = KeyboardKey::from_packet(pkt) {
                    match state.handle_shortcut(&key) {
                        Some(shortcut) => {
                            let ret = self.event_proxy.send_event(UserEvent::Shortcut(shortcut));
                            if let Err(err) = ret {
                                warn!("Fail to send shortcut event: {:?}", err);
                            }

                            RouterHookPacketResult {
                                process_event: false,
                            }
                        }
                        None => RouterHookPacketResult {
                            process_event: true,
                        },
                    }
                } else {
                    RouterHookPacketResult {
                        process_event: true,
                    }
                }
            }
            InputType::MouseButton => {
                if let Some(btn) = MouseButton::from_packet(pkt) {
                    if btn.button == MouseButtonType::Left && !btn.pressed {
                        _ = self.event_proxy.send_event(UserEvent::LeftButtonReleased);
                    }

                    state.handle_mouse_button(&btn);
                }

                RouterHookPacketResult {
                    process_event: true,
                }
            }

            _ => RouterHookPacketResult {
                process_event: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use mockall::predicate::*;
    use mockall::*;

    use kynput_rs::{CmpStruct, InputTarget, MousePosition, MouseWheel};

    use super::*;
    use crate::error::Result;
    use crate::event_loop::EventProxy;

    mock! {
        pub Proxy {
        }

        impl EventProxy for Proxy {
            fn send_event(&self, event: UserEvent) -> Result<()>;
        }
    }

    #[test]
    fn mouse_state() {
        let userevent_compare = move |e: &UserEvent| {
            if let UserEvent::LeftButtonReleased = e {
                true
            } else {
                false
            }
        };

        let mut proxy = MockProxy::new();
        proxy
            .expect_send_event()
            .with(predicate::function(userevent_compare))
            .times(1)
            .returning(|_| Ok(()));

        let hook = ClientPipelineHook::new(proxy);

        let state_reader = hook.get_state_reader();
        assert!(!state_reader.has_one_mouse_button_pressed());

        let ret_process_true = RouterHookPacketResult {
            process_event: true,
        };

        let mouse_btn = MouseButton {
            button: MouseButtonType::Left,
            pressed: true,
        };
        let ret = hook.on_packet(&mouse_btn.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));
        assert!(state_reader.has_one_mouse_button_pressed());

        let mouse_btn = MouseButton {
            button: MouseButtonType::Right,
            pressed: true,
        };
        let ret = hook.on_packet(&mouse_btn.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));
        assert!(state_reader.has_one_mouse_button_pressed());

        let mouse_btn = MouseButton {
            button: MouseButtonType::Right,
            pressed: false,
        };
        let ret = hook.on_packet(&mouse_btn.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));
        assert!(state_reader.has_one_mouse_button_pressed());

        let mouse_btn = MouseButton {
            button: MouseButtonType::Left,
            pressed: false,
        };
        let ret = hook.on_packet(&mouse_btn.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));
        assert!(!state_reader.has_one_mouse_button_pressed());
    }

    #[test]
    fn shortcuts() {
        let shortcut_event = ShortcutType::Fullscreen;
        let (shortcut, _) = SHORTCUTS
            .iter()
            .find(|(_, s)| *s == shortcut_event)
            .expect("Fullscreen shortcut must exist");
        let shortcut = shortcut.as_slice();

        let shortcut_compare = move |e: &UserEvent| {
            let UserEvent::Shortcut(e) = e else {
                return false;
            };

            *e == shortcut_event
        };

        let mut proxy = MockProxy::new();
        proxy
            .expect_send_event()
            .with(predicate::function(shortcut_compare))
            .times(1)
            .returning(|_| Ok(()));

        let hook = ClientPipelineHook::new(proxy);

        let ret_process_true = RouterHookPacketResult {
            process_event: true,
        };

        let shortcut_begin = &shortcut[0..shortcut.len() - 1];
        let shortcut_end = shortcut[shortcut.len() - 1];

        for &scancode in shortcut_begin {
            let key = KeyboardKey {
                scancode,
                pressed: true,
            };
            let ret = hook.on_packet(&key.to_packet(InputTarget::Client));
            assert!(ret.eq(&ret_process_true));
        }

        let key = KeyboardKey {
            scancode: shortcut_end,
            pressed: true,
        };
        let ret = hook.on_packet(&key.to_packet(InputTarget::Client));
        assert!(ret.eq(&RouterHookPacketResult {
            process_event: false,
        }));
    }

    #[test]
    fn unfiltered_events() {
        let proxy = MockProxy::new();
        let hook = ClientPipelineHook::new(proxy);

        let ret_process_true = RouterHookPacketResult {
            process_event: true,
        };

        let wheel = MouseWheel { dx: 1.0, dy: 2.0 };
        let ret = hook.on_packet(&wheel.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));

        let position = MousePosition {
            display_id: 43,
            x: 1,
            y: 2,
        };
        let ret = hook.on_packet(&position.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));
    }
}
