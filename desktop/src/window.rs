use std::sync::{Arc, Mutex};

use log::{debug, warn};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{self, CursorRgba, WindowBuilder},
};

use libclient_rs::Client;
use libkynput_rs::{Handler, InputType, MouseButton, Packet, Pipeline, RouterProcessingState};

use crate::error::{Error, Result};

use crate::client_proxy::ClientEventProxy;
use crate::cursor_handler::WindowCursorHandler;
use crate::pipeline_hook::{PipelineStateReader, ShortcutType};

use crate::video_layout::{Position, RectSize, VideoLayout};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UserEvent {
    StopRequest,
    Shortcut(ShortcutType),
    Client(libclient_rs::Event),
    SetCursor,
}

pub type StopRequestCb = Box<dyn Fn() + Send>;

struct State {
    client: Option<Client>,
    window: winit::window::Window,
    window_size: RectSize,
    input_pipeline: Option<Pipeline>,
    pipeline_state: PipelineStateReader,
    input_handler: Option<Arc<crate::input_handler::WinitHandler>>,
    video_layout: VideoLayout,

    window_focused: bool,
    cursor_inside_window: bool,

    cursor_visible: bool,
    cursor_handler: WindowCursorHandler,
}

impl State {
    fn mouse_button_filter_pressed(pkt: &Packet) -> bool {
        match MouseButton::from_packet(pkt) {
            Some(mouse_button) => !mouse_button.pressed,
            None => {
                warn!("Expected a MouseButton, got {:?}", pkt.get_type());
                false
            }
        }
    }

    fn handle_cursor_inside_window(&mut self, inside: bool) -> Result<()> {
        self.cursor_inside_window = inside;

        let pipeline = self.input_pipeline.as_ref().ok_or(Error::NoPipeline)?;

        let types = [InputType::MouseMove, InputType::MouseWheel];
        for t in types {
            pipeline.set_type_processing_state(t, RouterProcessingState::from_boolean(inside))?;
        }

        if inside {
            pipeline.set_type_processing_state(
                InputType::MouseButton,
                RouterProcessingState::from_boolean(true),
            )?;
        } else {
            pipeline.set_type_processing_state(
                InputType::MouseButton,
                RouterProcessingState::from_callback(Box::new(Self::mouse_button_filter_pressed)),
            )?;
        }

        Ok(())
    }

    fn stop_client(&mut self) -> Result<()> {
        let client = self.client.as_mut().ok_or(Error::NoClient)?;
        client.stop()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: &Event<UserEvent>,
        control_flow: &mut ControlFlow,
    ) -> Result<()> {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } => {
                if *window_id == self.window.id() {
                    self.stop_client()?;
                }
            }
            Event::UserEvent(UserEvent::StopRequest) => {
                self.stop_client()?;
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                self.window_focused = true;

                if !focused {
                    self.input_pipeline
                        .as_ref()
                        .ok_or(Error::NoPipeline)?
                        .flush_keys()?;
                }

                self.input_pipeline
                    .as_ref()
                    .ok_or(Error::NoPipeline)?
                    .set_global_processing_state(*focused)?;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorEntered { device_id: _ },
                ..
            } => {
                self.handle_cursor_inside_window(true)?;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorLeft { device_id: _ },
                ..
            } => {
                self.handle_cursor_inside_window(false)?;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                self.window_size.width = size.width;
                self.window_size.height = size.height;

                debug!("Window resized");
                self.video_layout.update_display_layout(RectSize {
                    width: size.width,
                    height: size.height,
                })
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                if let Some(input_handler) = self.input_handler.as_ref() {
                    let projected_pos = self.video_layout.local_to_host(
                        Position {
                            x: position.x as u32,
                            y: position.y as u32,
                        },
                        self.pipeline_state.has_one_mouse_button_pressed(),
                    );

                    if let Some(projected_pos) = projected_pos {
                        input_handler
                            .send_mouse_position(projected_pos.x as i16, projected_pos.y as i16);
                    }
                } else {
                    warn!("WindowEvent::CursorMoved discarded because no input handler is defined");
                }
            }
            Event::DeviceEvent { event, .. } => {
                if let Some(handler) = self.input_handler.as_ref() {
                    handler.consume_device_event(event);
                } else {
                    warn!("DeviceEvent discarded because no input handler is defined");
                }
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::Stopped)) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::Fullscreen)) => {
                if self.window.fullscreen().is_some() {
                    debug!("Exit fullscreen");
                    self.window.set_fullscreen(None);

                    let window_size = self.window_size;

                    self.video_layout.update_display_layout(RectSize {
                        width: window_size.width,
                        height: window_size.height,
                    });

                    if self.cursor_visible {
                        self.window.set_cursor_grab(false)?;
                    }
                } else {
                    debug!("Enter fullscreen");

                    let monitor = self.window.current_monitor();

                    if let Some(monitor) = monitor.as_ref() {
                        let monitor_size = monitor.size();

                        self.video_layout.update_display_layout(RectSize {
                            width: monitor_size.width,
                            height: monitor_size.height,
                        })
                    }

                    self.window
                        .set_fullscreen(Some(window::Fullscreen::Borderless(monitor)));

                    self.window.set_cursor_grab(true)?;
                }
            }
            Event::UserEvent(UserEvent::SetCursor) => {
                let cursor = self.cursor_handler.pop_cursor()?;

                // MousePosition and MouseMove are always produced on client side.
                // But only one must be sent, depending of the cursor visibility:
                // - Cursor visible: Send absolute position
                // - Cursor not visible: Send relative moves
                let pipeline = self.input_pipeline.as_ref().ok_or(Error::NoPipeline)?;

                if cursor.visible {
                    // Cursor must be restored before updating its shape.
                    // Otherwise on Linux, the default system cursor will be shown.
                    self.window.set_cursor_visible(true);

                    // If cursor wasn't previously visible, it must be moved at the position
                    // indicated by the Host. There is a drift between client cursor position,
                    // and host cursor position when injecting relative mouse move.
                    //
                    // To fix this issue, the Host sends the cursor position if known,
                    // otherwise i32::MAX is sent. Then the client must move its own cursor to
                    // match the host cursor coordinates.
                    if !self.cursor_visible && cursor.x != i32::MAX && cursor.y != i32::MAX {
                        use winit::dpi::PhysicalPosition;

                        let position = self.video_layout.host_to_local(Position {
                            x: cursor.x as u32,
                            y: cursor.y as u32,
                        });

                        self.window
                            .set_cursor_position(PhysicalPosition::new(position.x, position.y))?;
                    }

                    if !cursor.data.is_empty() {
                        self.window.set_cursor_rgba(CursorRgba {
                            xhot: cursor.xhot,
                            yhot: cursor.yhot,
                            width: cursor.width,
                            height: cursor.height,
                            data: cursor.data,
                        });
                    }

                    if self.window.fullscreen().is_none() {
                        self.window.set_cursor_grab(false)?;
                    }

                    pipeline.set_type_processing_state(
                        InputType::MousePosition,
                        RouterProcessingState::from_boolean(true),
                    )?;
                    pipeline.set_type_processing_state(
                        InputType::MouseMove,
                        RouterProcessingState::from_boolean(false),
                    )?;
                } else {
                    self.window.set_cursor_visible(false);
                    self.window.set_cursor_grab(true)?;

                    pipeline.set_type_processing_state(
                        InputType::MousePosition,
                        RouterProcessingState::from_boolean(false),
                    )?;
                    pipeline.set_type_processing_state(
                        InputType::MouseMove,
                        RouterProcessingState::from_boolean(true),
                    )?;
                }

                self.cursor_visible = cursor.visible;
            }
            _ => {}
        }

        Ok(())
    }
}

pub struct Window {
    event_loop: EventLoop<UserEvent>,
    state: Arc<Mutex<State>>,
    window_handle: RawWindowHandle,
}

impl Window {
    pub fn new(
        event_loop: EventLoop<UserEvent>,
        pipeline_state: PipelineStateReader,
    ) -> Result<Self> {
        // Create Loop + Window
        let window_width: u32 = 800;
        let window_height: u32 = 600;

        let window = WindowBuilder::new()
            .with_title("Kyber Client")
            .with_inner_size(winit::dpi::PhysicalSize::new(window_width, window_height))
            .build(&event_loop)?;

        let mut video_layout = VideoLayout {
            window_size: RectSize {
                width: window_width,
                height: window_height,
            },
            host_size: RectSize {
                width: 1920,
                height: 1080,
            },
            video_size: RectSize {
                width: 0,
                height: 0,
            },
            video_pos: Position { x: 0, y: 0 },
        };

        video_layout.update_display_layout(RectSize {
            width: window_width,
            height: window_height,
        });

        Ok(Self {
            event_loop,
            window_handle: window.raw_window_handle(),
            state: Arc::new(Mutex::new(State {
                client: None,
                window,
                input_pipeline: None,
                pipeline_state,
                input_handler: None,
                window_size: RectSize {
                    width: window_width,
                    height: window_height,
                },
                video_layout,
                window_focused: false,
                cursor_inside_window: false,
                cursor_visible: true,
                cursor_handler: WindowCursorHandler::new(),
            })),
        })
    }

    pub fn create_stop_request_cb(&mut self) -> StopRequestCb {
        let proxy = self.event_loop.create_proxy();

        Box::new(move || {
            proxy.send_event(UserEvent::StopRequest).unwrap();
        })
    }

    pub fn create_cursor_handler(&mut self) -> Result<Handler> {
        let state = self.state.lock()?;
        Ok(state
            .cursor_handler
            .create_proxy(self.event_loop.create_proxy()))
    }

    pub fn set_input_handler(
        &mut self,
        handler: Arc<crate::input_handler::WinitHandler>,
    ) -> Result<()> {
        let mut state = self.state.lock()?;
        state.input_handler = Some(handler);
        Ok(())
    }

    pub fn set_client(&mut self, client: Client) -> Result<()> {
        let mut state = self.state.lock()?;

        {
            let client_proxy = Box::new(ClientEventProxy::new(self.event_loop.create_proxy()));
            client.set_event_handler(client_proxy)?;
        }

        state.client = Some(client);

        Ok(())
    }

    pub fn set_input_pipeline(&self, input_pipeline: Pipeline) -> Result<()> {
        let mut state = self.state.lock()?;

        // Assume that the default cursor is visible at startup
        input_pipeline.set_type_processing_state(
            InputType::MouseMove,
            RouterProcessingState::from_boolean(false),
        )?;

        state.input_pipeline = Some(input_pipeline);

        Ok(())
    }

    pub fn window_handle(&self) -> RawWindowHandle {
        self.window_handle
    }

    pub fn run_loop(self) -> Result<()> {
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match self.state.lock() {
                Ok(mut state) => {
                    if let Err(err) = state.handle_event(&event, control_flow) {
                        warn!(
                            "Event handler failed during processing of {:?}: {:?}",
                            event, err
                        );
                    }
                }
                Err(err) => {
                    warn!("Fail to lock state: {:?}", err);
                }
            }
        });
    }
}
