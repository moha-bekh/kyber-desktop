use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use log::{debug, warn};
use winit::event_loop::EventLoopProxy;

use libkynput_rs::{InputType, KeyboardKey, MouseButton, MouseButtonType, Packet};
use libkynput_rs::{RouterHook, RouterHookPacketResult};

use crate::error::{Error, Result};
use crate::window::UserEvent;

const SCANCODE_UNKNOWN: u16 = 0;
const SCANCODE_F: u16 = 33;
const SCANCODE_LCTRL: u16 = 29;
const SCANCODE_LALT: u16 = 56;

// Taken from SDL
const SCANCODE_COUNT: usize = 512;

const SHORTCUT_FULLSCREEN: &[u16] = &[SCANCODE_F, SCANCODE_LCTRL, SCANCODE_LALT];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutType {
    Fullscreen,
}

struct InternalState {
    scancodes: [bool; SCANCODE_COUNT],
    mouse_btns: HashSet<MouseButtonType>,
}

impl InternalState {
    fn new() -> Self {
        Self {
            scancodes: [false; SCANCODE_COUNT],
            mouse_btns: HashSet::new(),
        }
    }

    fn check_shortcut(&self, scancodes: &[u16]) -> bool {
        scancodes.iter().all(|s| self.scancodes[*s as usize])
    }

    fn handle_shortcut(&mut self, event: &KeyboardKey) -> Option<ShortcutType> {
        // Update keyboard state
        if event.scancode != SCANCODE_UNKNOWN {
            if event.scancode < self.scancodes.len() as u16 {
                self.scancodes[event.scancode as usize] = event.pressed;
            } else {
                debug!("Ignore scancode {}", event.scancode);
            }
        }

        if self.check_shortcut(SHORTCUT_FULLSCREEN) {
            Some(ShortcutType::Fullscreen)
        } else {
            None
        }
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
}

trait WindowProxy: Send {
    fn send_event(&self, event: UserEvent) -> Result<()>;
}

struct WindowProxyImpl {
    window_proxy: EventLoopProxy<UserEvent>,
}

impl WindowProxy for WindowProxyImpl {
    fn send_event(&self, event: UserEvent) -> Result<()> {
        self.window_proxy.send_event(event).map_err(|e| {
            warn!("Send failed: {:?}", e);
            Error::Rejected
        })?;

        Ok(())
    }
}

pub struct ClientPipelineHook {
    window_proxy: Box<dyn WindowProxy>,
    state: Arc<Mutex<InternalState>>,
}

impl ClientPipelineHook {
    pub fn new_from_event_loop(window_proxy: EventLoopProxy<UserEvent>) -> Self {
        Self::new(Box::new(WindowProxyImpl { window_proxy }))
    }

    fn new(window_proxy: Box<dyn WindowProxy>) -> Self {
        Self {
            window_proxy,
            state: Arc::new(Mutex::new(InternalState::new())),
        }
    }

    pub fn get_state_reader(&self) -> PipelineStateReader {
        PipelineStateReader {
            state: self.state.clone(),
        }
    }
}

impl RouterHook for ClientPipelineHook {
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
                            let ret = self.window_proxy.send_event(UserEvent::Shortcut(shortcut));
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

    mock! {
        Proxy {
        }

        impl WindowProxy for Proxy {
            fn send_event(&self, event: UserEvent) -> Result<()>;
        }
    }

    #[test]
    fn mouse_state() {
        let proxy = Box::new(MockProxy::new());

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

        let mut proxy = Box::new(MockProxy::new());
        proxy
            .expect_send_event()
            .with(predicate::eq(UserEvent::Shortcut(shortcut_event)))
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
        let proxy = Box::new(MockProxy::new());
        let hook = ClientPipelineHook::new(proxy);

        let ret_process_true = RouterHookPacketResult {
            process_event: true,
        };

        let wheel = MouseWheel { dx: 1.0, dy: 2.0 };
        let ret = hook.on_packet(&wheel.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));

        let position = MousePosition { x: 1, y: 2 };
        let ret = hook.on_packet(&position.to_packet(InputTarget::Client));
        assert!(ret.eq(&ret_process_true));
    }
}
