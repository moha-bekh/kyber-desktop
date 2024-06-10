use std::sync::Arc;

use log::warn;

use libkynput_rs::{
    ConsumerInterface, Cursor, CursorFormat, Handler, HandlerConsumerInterface, HandlerInterface,
    InputType, InputTypeMetadata, Packet,
};

use crate::error::Result;
use crate::event_loop::{EventProxy, UserEvent};

struct WindowCursorHandler<T>
where
    T: EventProxy,
{
    event_loop_proxy: T,
}

impl<T> WindowCursorHandler<T>
where
    T: EventProxy,
{
    pub fn new(event_loop_proxy: T) -> Self {
        Self { event_loop_proxy }
    }
}

impl<T> HandlerConsumerInterface for WindowCursorHandler<T>
where
    T: EventProxy,
{
    fn handled_types(&self) -> Vec<InputType> {
        vec![InputType::Cursor]
    }
}

impl<T> ConsumerInterface for WindowCursorHandler<T>
where
    T: EventProxy,
{
    fn consume(&self, pkt: Packet) {
        let Some(cursor) = Cursor::from_packet(&pkt) else {
            return;
        };

        self.event_loop_proxy
            .send_event(UserEvent::SetCursor(cursor))
            .unwrap();
    }
}

impl<T> HandlerInterface for WindowCursorHandler<T>
where
    T: EventProxy,
{
    fn start(&self) {}

    fn stop(&self) {}

    fn query_type_metadata(&self, type_: InputType) -> Option<InputTypeMetadata> {
        match type_ {
            InputType::Cursor => Some(InputTypeMetadata::Cursor {
                format: CursorFormat::Raw,
            }),
            _ => {
                warn!("Got query_type_metadata() for unexpected type: {type_:?}");
                None
            }
        }
    }
}

pub fn new_cursor_handler<T>(event_loop_proxy: T) -> Result<Handler>
where
    T: EventProxy,
{
    let consumer = Arc::new(WindowCursorHandler::new(event_loop_proxy));
    Ok(libkynput_rs::create_handler_consumer_instance(consumer))
}
