use std::sync::Arc;

use winit::event_loop::EventLoopProxy;

use libkynput_rs::{
    ConsumerInterface, Cursor, Handler, HandlerConsumerInterface, HandlerInterface, InputType,
    Packet,
};

use crate::error::Result;
use crate::window::UserEvent;

struct WindowCursorHandler {
    event_loop_proxy: EventLoopProxy<UserEvent>,
}

impl WindowCursorHandler {
    pub fn new(event_loop_proxy: EventLoopProxy<UserEvent>) -> Self {
        Self { event_loop_proxy }
    }
}

impl HandlerConsumerInterface for WindowCursorHandler {
    fn handled_types(&self) -> Vec<InputType> {
        vec![InputType::Cursor]
    }
}

impl ConsumerInterface for WindowCursorHandler {
    fn consume(&self, pkt: Packet) {
        let Some(cursor) = Cursor::from_packet(pkt) else {
            return;
        };

        self.event_loop_proxy
            .send_event(UserEvent::SetCursor(cursor))
            .unwrap();
    }
}

impl HandlerInterface for WindowCursorHandler {
    fn start(&self) {}

    fn stop(&self) {}
}

pub fn new_cursor_handler(event_loop_proxy: EventLoopProxy<UserEvent>) -> Result<Handler> {
    let consumer = Arc::new(WindowCursorHandler::new(event_loop_proxy));
    Ok(libkynput_rs::create_handler_consumer_instance(consumer))
}
