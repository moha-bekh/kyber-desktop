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

use libclient_rs::Display;
use libkynput_rs::{InputTarget, InputType};

use super::*;
use crate::event_loop::UserEvent;
use crate::window::Size;

struct InputProducer {
    consumer: libkynput_rs::Consumer,
}

impl InputProducer {
    fn new(consumer: libkynput_rs::Consumer) -> Self {
        Self { consumer }
    }
}

impl libkynput_rs::HandlerInterface for InputProducer {
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

    impl libkynput_rs::ConsumerInterface for InputNetwork {
        fn consume(&self, packet: libkynput_rs::Packet);
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
    }
}
