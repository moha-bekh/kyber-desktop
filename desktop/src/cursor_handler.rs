use std::sync::{Arc, Mutex};

use winit::event_loop::EventLoopProxy;

use libkynput_rs::{
    ConsumerInterface, Cursor, Handler, HandlerConsumerInterface, HandlerInterface, InputType,
    Packet,
};

use crate::error::{Error, Result};
use crate::window::UserEvent;

pub struct WindowCursorHandler {
    cursor: Arc<Mutex<Option<Cursor>>>,
}

impl WindowCursorHandler {
    pub fn new() -> Self {
        Self {
            cursor: Arc::new(Mutex::new(None)),
        }
    }

    pub fn pop_cursor(&self) -> Result<Cursor> {
        if let Some(cursor) = self.cursor.lock()?.take() {
            Ok(cursor)
        } else {
            Err(Error::Rejected)
        }
    }

    pub fn create_proxy(&self, event_loop_proxy: EventLoopProxy<UserEvent>) -> Handler {
        let proxy = Arc::new(WindowCursorProxy::new(
            event_loop_proxy,
            self.cursor.clone(),
        ));
        libkynput_rs::create_handler_consumer_instance(proxy)
    }
}

pub struct WindowCursorProxy {
    event_loop_proxy: Mutex<EventLoopProxy<UserEvent>>,
    cursor: Arc<Mutex<Option<Cursor>>>,
}

impl WindowCursorProxy {
    fn new(
        event_loop_proxy: EventLoopProxy<UserEvent>,
        cursor: Arc<Mutex<Option<Cursor>>>,
    ) -> Self {
        Self {
            event_loop_proxy: Mutex::new(event_loop_proxy),
            cursor,
        }
    }
}

impl HandlerConsumerInterface for WindowCursorProxy {
    fn handled_types(&self) -> Vec<InputType> {
        vec![InputType::Cursor]
    }
}

impl ConsumerInterface for WindowCursorProxy {
    fn consume(&self, pkt: Packet) {
        let cursor = if let Some(cursor) = Cursor::from_packet(pkt) {
            cursor
        } else {
            return;
        };

        let mut state_cursor = self.cursor.lock().unwrap();
        *state_cursor = Some(cursor);

        let event_loop_proxy = self.event_loop_proxy.lock().unwrap();
        event_loop_proxy.send_event(UserEvent::SetCursor).unwrap();
    }
}

impl HandlerInterface for WindowCursorProxy {
    fn start(&self) {}

    fn stop(&self) {}
}
