use winit::event_loop::EventLoopProxy;

use libclient_rs::{Event, EventSink};

use super::UserEvent;

pub struct ClientEventProxy {
    proxy: EventLoopProxy<UserEvent>,
}

impl ClientEventProxy {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self { proxy }
    }
}

impl EventSink for ClientEventProxy {
    fn on_event(&self, event: Event) {
        self.proxy.send_event(UserEvent::Client(event)).unwrap();
    }
}
