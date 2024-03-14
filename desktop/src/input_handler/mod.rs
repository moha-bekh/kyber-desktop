#[cfg(not(test))]
#[cfg_attr(target_os = "linux", path = "./linux.rs")]
#[cfg_attr(target_os = "macos", path = "./macos.rs")]
#[cfg_attr(target_os = "windows", path = "./windows.rs")]
mod platform;

#[cfg(test)]
use tests::platform;

use std::collections::{HashMap, HashSet};

use log::warn;
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceEvent, MouseScrollDelta, WindowEvent},
};

use crate::error::Result;

use libkynput_rs::{
    Consumer, HandlerInterface, InputTarget, InputType, KeyboardKey, MouseButton, MouseButtonType,
    MouseMove, MousePosition, MouseWheel,
};

#[derive(Debug, PartialEq)]
enum WinitEventType {
    Device,
    Window,
}

trait PlatformImpl {
    fn handled_events() -> HashMap<InputType, WinitEventType>;

    fn convert_scancode(scancode: u16) -> Option<u16>;

    fn convert_device_mouse_button(_button: u32) -> Option<MouseButtonType> {
        None
    }

    fn convert_window_mouse_button(_button: &winit::event::MouseButton) -> Option<MouseButtonType> {
        None
    }

    fn convert_mouse_wheel(horizontal: f32, vertical: f32) -> MouseWheel;

    fn convert_mouse_wheel_pixel_delta(
        _position: &PhysicalPosition<f64>,
        _scale_factor: f64,
    ) -> Option<MouseWheel> {
        None
    }
}

fn winit_deviceevent_to_libkynput(event: &DeviceEvent) -> Option<InputType> {
    match event {
        DeviceEvent::Key(_) => Some(InputType::Keyboard),
        DeviceEvent::Button { .. } => Some(InputType::MouseButton),
        DeviceEvent::MouseWheel { .. } => Some(InputType::MouseWheel),
        DeviceEvent::MouseMotion { .. } => Some(InputType::MouseMove),
        _ => None,
    }
}

fn winit_windowevent_to_libkynput(event: &WindowEvent) -> Option<InputType> {
    match event {
        WindowEvent::KeyboardInput { .. } => Some(InputType::Keyboard),
        WindowEvent::MouseInput { .. } => Some(InputType::MouseButton),
        WindowEvent::MouseWheel { .. } => Some(InputType::MouseWheel),
        _ => None,
    }
}

fn convert_element_state(state: winit::event::ElementState) -> bool {
    match state {
        winit::event::ElementState::Pressed => true,
        winit::event::ElementState::Released => false,
    }
}

pub struct WinitHandler {
    handled_event_map: HashMap<InputType, WinitEventType>,
    produced_types: Vec<InputType>,

    consumer: Consumer,
}

impl WinitHandler {
    pub fn producable_types() -> Vec<InputType> {
        // Position is handled directly by the generic code
        let mut types = vec![InputType::MousePosition];

        let mut platform_types: Vec<_> = platform::Impl::handled_events().keys().copied().collect();
        types.append(&mut platform_types);

        types
    }

    pub fn new(consumer: Consumer, requested_produced_types: Vec<InputType>) -> Result<Self> {
        // The InputType produced by the handler depends on:
        // - The InputType that can be produced by the platform implementation
        // - The InputType list requested by the caller
        //
        // The produced list is the intersection of those two lists.
        let mut full_requested_produced_types: HashSet<InputType> =
            requested_produced_types.iter().copied().collect();
        full_requested_produced_types.insert(InputType::MousePosition);

        let producable_set: HashSet<InputType> = Self::producable_types().iter().copied().collect();

        Ok(Self {
            handled_event_map: platform::Impl::handled_events(),
            produced_types: full_requested_produced_types
                .intersection(&producable_set)
                .copied()
                .collect(),
            consumer,
        })
    }

    fn can_process_event(
        &self,
        libkynput_type: Option<InputType>,
        winit_type: WinitEventType,
    ) -> bool {
        // Unpack the InputType if valid
        if let Some(libkynput_type) = libkynput_type {
            // Filter events that shouldn't be produced here
            if !self.produced_types.contains(&libkynput_type) {
                return false;
            }

            // Check if the winit event type can be processed for this platform
            match self.handled_event_map.get(&libkynput_type) {
                Some(expected_type) => *expected_type == winit_type,
                None => {
                    warn!("WinitEventType {:?} not found in platform map", winit_type);
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn consume_device_event(&self, event: &DeviceEvent) {
        // Check if the DeviceEvent type should be handled by this producer
        if !self.can_process_event(
            winit_deviceevent_to_libkynput(event),
            WinitEventType::Device,
        ) {
            return;
        }

        // Convert and send the event
        let pkt = match event {
            DeviceEvent::Key(event) => {
                match platform::Impl::convert_scancode(event.scancode as u16) {
                    Some(scancode) => {
                        let pressed = convert_element_state(event.state);
                        let key = KeyboardKey { scancode, pressed };
                        key.to_packet(InputTarget::Host)
                    }
                    None => return,
                }
            }
            DeviceEvent::Button { button, state } => {
                match platform::Impl::convert_device_mouse_button(*button) {
                    Some(button) => {
                        let pressed = convert_element_state(*state);
                        let button = MouseButton { button, pressed };
                        button.to_packet(InputTarget::Host)
                    }
                    None => return,
                }
            }
            DeviceEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(horizontal, vertical),
            } => {
                let mouse_wheel = platform::Impl::convert_mouse_wheel(*horizontal, *vertical);
                mouse_wheel.to_packet(InputTarget::Host)
            }
            DeviceEvent::MouseMotion { delta: (x, y) } => {
                let mouse_move = MouseMove {
                    dx: *x as i16,
                    dy: *y as i16,
                };
                mouse_move.to_packet(InputTarget::Host)
            }
            _ => {
                warn!("Unhandled DeviceEvent {:?}", event);
                return;
            }
        };

        self.consumer.consume(pkt);
    }

    pub fn send_mouse_position(&self, display_id: u32, x: i16, y: i16) {
        let position = MousePosition { display_id, x, y };
        let pkt = position.to_packet(InputTarget::Host);
        self.consumer.consume(pkt);
    }

    pub fn consume_window_event(&self, event: &WindowEvent, scale_factor: f64) {
        // Check if the DeviceEvent type should be handled by this producer
        if !self.can_process_event(
            winit_windowevent_to_libkynput(event),
            WinitEventType::Window,
        ) {
            return;
        }

        let pkt = match event {
            WindowEvent::KeyboardInput { input, .. } => {
                match platform::Impl::convert_scancode(input.scancode as u16) {
                    Some(scancode) => {
                        let pressed = convert_element_state(input.state);
                        let key = KeyboardKey { scancode, pressed };
                        key.to_packet(InputTarget::Host)
                    }
                    None => {
                        warn!("Ignoring keycode {}", input.scancode);
                        return;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some(btn) = platform::Impl::convert_window_mouse_button(button) else {
                    return;
                };

                let button = MouseButton {
                    button: btn,
                    pressed: convert_element_state(*state),
                };
                button.to_packet(InputTarget::Host)
            }
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(horizontal, vertical),
                ..
            } => {
                let wheel = platform::Impl::convert_mouse_wheel(*horizontal, *vertical);
                wheel.to_packet(InputTarget::Host)
            }
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::PixelDelta(position),
                ..
            } => {
                let wheel = platform::Impl::convert_mouse_wheel_pixel_delta(position, scale_factor);

                if let Some(wheel) = wheel {
                    wheel.to_packet(InputTarget::Host)
                } else {
                    return;
                }
            }
            _ => return,
        };

        self.consumer.consume(pkt);
    }
}

impl HandlerInterface for WinitHandler {
    fn start(&self) {}

    fn stop(&self) {}

    fn produced_types(&self) -> Option<Vec<InputType>> {
        let producable_types = WinitHandler::producable_types();

        if !producable_types.is_empty() {
            Some(producable_types)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Eq;
    use std::collections::HashSet;
    use std::fmt::Debug;
    use std::hash::Hash;
    use std::sync::Arc;

    use mockall::predicate::*;
    use mockall::*;
    use serial_test::serial;
    use winit::event::{ElementState, KeyboardInput, ModifiersState};

    use libkynput_rs::{ConsumerInterface, InputType, MouseButton, Packet};

    use super::*;

    mock! {
        Consumer {
        }

        impl ConsumerInterface for Consumer {
            fn consume(&self, pkt: Packet);
        }
    }

    pub mod platform {
        use std::collections::HashMap;

        use super::WinitEventType;
        use libkynput_rs::{InputType, MouseButtonType, MouseWheel};

        pub static mut PROVIDE_ALL_EVENTS: bool = false;

        pub struct Impl {}

        impl super::PlatformImpl for Impl {
            fn handled_events() -> HashMap<InputType, WinitEventType> {
                unsafe {
                    if PROVIDE_ALL_EVENTS {
                        HashMap::from([
                            (InputType::Keyboard, WinitEventType::Device),
                            (InputType::MouseButton, WinitEventType::Device),
                            (InputType::MouseMove, WinitEventType::Device),
                            (InputType::MouseWheel, WinitEventType::Device),
                        ])
                    } else {
                        HashMap::from([
                            (InputType::Keyboard, WinitEventType::Device),
                            (InputType::MouseButton, WinitEventType::Device),
                        ])
                    }
                }
            }

            fn convert_scancode(scancode: u16) -> Option<u16> {
                if scancode == 43 {
                    None
                } else {
                    Some(scancode + 1)
                }
            }

            fn convert_mouse_wheel(horizontal: f32, vertical: f32) -> MouseWheel {
                MouseWheel {
                    dx: horizontal + 2.0,
                    dy: vertical + 2.0,
                }
            }

            fn convert_device_mouse_button(button: u32) -> Option<MouseButtonType> {
                match button {
                    1 => Some(MouseButtonType::Left),
                    2 => Some(MouseButtonType::Middle),
                    3 => Some(MouseButtonType::Right),
                    _ => None,
                }
            }
        }
    }

    fn compare_vectors<T>(a: Vec<T>, b: Vec<T>)
    where
        T: Copy + Debug + Eq + Hash,
    {
        assert_eq!(
            a.iter().copied().collect::<HashSet<T>>(),
            b.iter().copied().collect::<HashSet<T>>()
        );
    }

    #[test]
    #[serial]
    fn producable_types() {
        // InputType::MousePosition is always producable
        unsafe { platform::PROVIDE_ALL_EVENTS = false };
        compare_vectors(
            WinitHandler::producable_types(),
            vec![
                InputType::Keyboard,
                InputType::MouseButton,
                InputType::MousePosition,
            ],
        );
    }

    #[test]
    #[serial]
    fn produced_types() {
        unsafe {
            platform::PROVIDE_ALL_EVENTS = false;
        }
        let consumer = MockConsumer::new();

        // MouseMove is producable, but not enabled
        // MouseButton is requested, but not available
        let produced_events = vec![
            InputType::Keyboard,
            InputType::MouseButton,
            InputType::MousePosition,
        ];

        let expected_produced_events = vec![
            InputType::Keyboard,
            InputType::MouseButton,
            InputType::MouseMove,
            InputType::MousePosition,
        ];

        let consumer = libkynput_rs::create_consumer_instance(Arc::new(consumer));

        let handler =
            WinitHandler::new(consumer.clone(), expected_produced_events.clone()).unwrap();
        compare_vectors(handler.produced_types, produced_events);
    }

    #[test]
    #[serial]
    fn mouse_position() {
        let mut consumer = MockConsumer::new();

        let move_position = MousePosition {
            display_id: 43,
            x: 1,
            y: 2,
        };
        let move_position_pkt = move_position.to_packet(InputTarget::Host);

        consumer
            .expect_consume()
            .with(predicate::eq(move_position_pkt))
            .times(1)
            .returning(|_| ());

        let consumer = libkynput_rs::create_consumer_instance(Arc::new(consumer));

        let produced_events = vec![InputType::Keyboard, InputType::MouseMove];
        let handler = WinitHandler::new(consumer.clone(), produced_events.clone()).unwrap();
        handler.send_mouse_position(43, 1, 2);
    }

    #[test]
    #[serial]
    fn device_event_mapping() {
        unsafe { platform::PROVIDE_ALL_EVENTS = true };

        // Setup consumer
        let mut consumer = MockConsumer::new();

        let key = KeyboardKey {
            scancode: 11,
            pressed: true,
        };
        let key_pkt = key.to_packet(InputTarget::Host);
        consumer
            .expect_consume()
            .with(predicate::eq(key_pkt))
            .times(1)
            .returning(|_| ());

        let mouse_btn = MouseButton {
            button: MouseButtonType::Middle,
            pressed: false,
        };
        let mouse_btn_pkt = mouse_btn.to_packet(InputTarget::Host);
        consumer
            .expect_consume()
            .with(predicate::eq(mouse_btn_pkt))
            .times(1)
            .returning(|_| ());

        let mouse_move = MouseMove { dx: 3, dy: 4 };
        let mouse_move_pkt = mouse_move.to_packet(InputTarget::Host);
        consumer
            .expect_consume()
            .with(predicate::eq(mouse_move_pkt))
            .times(1)
            .returning(|_| ());

        let mouse_wheel = MouseWheel { dx: 12.0, dy: 13.0 };
        let mouse_wheel_pkt = mouse_wheel.to_packet(InputTarget::Host);
        consumer
            .expect_consume()
            .with(predicate::eq(mouse_wheel_pkt))
            .times(1)
            .returning(|_| ());

        let consumer = libkynput_rs::create_consumer_instance(Arc::new(consumer));

        // Setup handler
        let produced_events = vec![
            InputType::MousePosition,
            InputType::Keyboard,
            InputType::MouseButton,
            InputType::MouseMove,
            InputType::MouseWheel,
        ];
        let handler = WinitHandler::new(consumer.clone(), produced_events.clone()).unwrap();

        // Run tests
        #[allow(deprecated)]
        handler.consume_device_event(&DeviceEvent::Key(KeyboardInput {
            scancode: 10,
            state: ElementState::Pressed,
            virtual_keycode: None,
            modifiers: ModifiersState::default(),
        }));

        #[allow(deprecated)]
        handler.consume_device_event(&DeviceEvent::Key(KeyboardInput {
            scancode: 43,
            state: ElementState::Pressed,
            virtual_keycode: None,
            modifiers: ModifiersState::default(),
        }));

        handler.consume_device_event(&DeviceEvent::Button {
            button: 2,
            state: ElementState::Released,
        });

        handler.consume_device_event(&DeviceEvent::Button {
            button: 43,
            state: ElementState::Released,
        });

        handler.consume_device_event(&DeviceEvent::MouseMotion { delta: (3.0, 4.0) });

        handler.consume_device_event(&DeviceEvent::MouseWheel {
            delta: MouseScrollDelta::LineDelta(10.0, 11.0),
        });

        // Not handled by the implementation
        handler.consume_device_event(&DeviceEvent::Text { codepoint: 'A' });
    }

    #[test]
    #[serial]
    fn device_event_mapping_filtered() {
        unsafe { platform::PROVIDE_ALL_EVENTS = false };

        // Setup consumer
        let mut consumer = MockConsumer::new();

        let key = KeyboardKey {
            scancode: 11,
            pressed: true,
        };
        let key_pkt = key.to_packet(InputTarget::Host);
        consumer
            .expect_consume()
            .with(predicate::eq(key_pkt))
            .times(1)
            .returning(|_| ());

        let mouse_btn = MouseButton {
            button: MouseButtonType::Middle,
            pressed: true,
        };
        let mouse_btn_pkt = mouse_btn.to_packet(InputTarget::Host);
        consumer
            .expect_consume()
            .with(predicate::eq(mouse_btn_pkt))
            .times(1)
            .returning(|_| ());

        let consumer = libkynput_rs::create_consumer_instance(Arc::new(consumer));

        // Setup handler
        let produced_events = vec![
            InputType::MousePosition,
            InputType::Keyboard,
            InputType::MouseButton,
        ];
        let handler = WinitHandler::new(consumer.clone(), produced_events.clone()).unwrap();

        // Run tests
        #[allow(deprecated)]
        handler.consume_device_event(&DeviceEvent::Key(KeyboardInput {
            scancode: 10,
            state: ElementState::Pressed,
            virtual_keycode: None,
            modifiers: ModifiersState::default(),
        }));

        handler.consume_device_event(&DeviceEvent::Button {
            button: 2,
            state: ElementState::Pressed,
        });

        handler.consume_device_event(&DeviceEvent::MouseMotion { delta: (3.0, 4.0) });

        handler.consume_device_event(&DeviceEvent::MouseWheel {
            delta: MouseScrollDelta::LineDelta(10.0, 11.0),
        });
    }
}
