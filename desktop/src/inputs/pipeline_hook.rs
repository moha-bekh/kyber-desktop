use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use log::warn;

use libkynput_rs::{InputType, KeyboardKey, MouseButton, MouseButtonType, Packet};
use libkynput_rs::{RouterHook, RouterHookPacketResult};

use crate::event_loop::{EventProxy, UserEvent};
use crate::inputs::winit_handler::scancodes::*;

const SHORTCUT_FULLSCREEN: &[u16] = &[SCANCODE_F, SCANCODE_LCTRL, SCANCODE_LALT];
const SHORTCUT_RESTART_STREAM: &[u16] = &[SCANCODE_R, SCANCODE_LCTRL, SCANCODE_LALT];
const SHORTCUT_TOGGLE_MUTE: &[u16] = &[SCANCODE_M, SCANCODE_LCTRL];
const SHORTCUT_BITRATE_UP: &[u16] = &[SCANCODE_UP, SCANCODE_LCTRL, SCANCODE_B];
const SHORTCUT_BITRATE_DOWN: &[u16] = &[SCANCODE_DOWN, SCANCODE_LCTRL, SCANCODE_B];
const SHORTCUT_VOLUME_UP: &[u16] = &[SCANCODE_UP, SCANCODE_LCTRL];
const SHORTCUT_VOLUME_DOWN: &[u16] = &[SCANCODE_DOWN, SCANCODE_LCTRL];

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

const SHORTCUT_MAP: &[(&[u16], ShortcutType)] = &[
    (SHORTCUT_FULLSCREEN, ShortcutType::Fullscreen),
    (SHORTCUT_RESTART_STREAM, ShortcutType::RestartStreaming),
    (SHORTCUT_TOGGLE_MUTE, ShortcutType::ToggleMute),
    (SHORTCUT_VOLUME_UP, ShortcutType::VolumeUp),
    (SHORTCUT_VOLUME_DOWN, ShortcutType::VolumeDown),
    (SHORTCUT_BITRATE_UP, ShortcutType::BitrateUp),
    (SHORTCUT_BITRATE_DOWN, ShortcutType::BitrateDown),
];

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
        SHORTCUT_MAP.iter().find_map(|(scancodes, shortcut)| {
            // Ignore shortcuts with a mismatching length
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

    use libkynput_rs::{CmpStruct, InputTarget, MousePosition, MouseWheel};

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
        let shortcut = SHORTCUT_FULLSCREEN;
        let shortcut_event = ShortcutType::Fullscreen;

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

        let shortcut_begin = &SHORTCUT_FULLSCREEN[0..shortcut.len() - 1];
        let shortcut_end = SHORTCUT_FULLSCREEN[shortcut.len() - 1];

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
