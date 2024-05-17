use std::cell::RefCell;
use std::sync::Arc;

use log::{debug, info, warn};
use raw_window_handle::HasRawWindowHandle;
use winit::{
    dpi::PhysicalPosition,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::{self, CursorGrabMode, CursorRgba, WindowBuilder},
};

use libclient_rs::{Client, ConnectConfig, PlayerConfig, StreamingSessionConfig, WindowHandle};
use libkynput_rs::{InputType, MouseButton, Packet, Pipeline, RouterProcessingState};

use crate::client_proxy::ClientEventProxy;
use crate::cursor_handler;
use crate::error::{Error, Result};
use crate::input_handler;
use crate::pipeline_hook::{PipelineStateReader, ShortcutType};
use crate::video_layout::{Position, RectSize, VideoLayout};
use crate::{CliArgs, ClientPipelineHook};

#[derive(Clone, Debug)]
pub enum UserEvent {
    Start,
    StopRequest,
    Shortcut(ShortcutType),
    Client(libclient_rs::Event),
    SetCursor(libkynput_rs::Cursor),
}

pub type StopRequestCb = Box<dyn Fn() + Send>;

struct InputContext {
    pipeline: Pipeline,
    state_reader: PipelineStateReader,
    winit_handler: Option<Arc<crate::input_handler::WinitHandler>>,
    supports_relative_mouse: bool,
}

struct State {
    eventloop_proxy: EventLoopProxy<UserEvent>,
    cli_args: CliArgs,
    client: Option<Client>,
    window: winit::window::Window,
    window_size: RectSize,
    focused: bool,
    video_layout: VideoLayout,

    display_id: Option<u32>,

    input_ctx: Option<InputContext>,
    last_cursor_pos: Option<PhysicalPosition<i32>>,

    cursor_inside_window: bool,

    cursor_visible: bool,
}

impl State {
    fn create_input_context(&mut self) -> Result<()> {
        // Create InputPipeline
        let pipeline_config = libkynput_rs::Config::default_client();
        let input_pipeline = Pipeline::new(pipeline_config);
        let cursor_handler = cursor_handler::new_cursor_handler(self.eventloop_proxy.clone())?;

        input_pipeline.plug_handler(cursor_handler)?;

        let missing_types: Vec<InputType> = input_handler::WinitHandler::producable_types()
            .iter()
            .filter(|&&t| !input_pipeline.produces_type(t))
            .copied()
            .collect();

        let winit_handler = if !missing_types.is_empty() {
            info!("Winit will be used to produce types {:?}", missing_types);
            let winit_handler = Arc::new(input_handler::WinitHandler::new(
                input_pipeline.get_input_sink(),
                missing_types,
            )?);

            input_pipeline
                .plug_handler(libkynput_rs::create_handler_instance(winit_handler.clone()))?;

            Some(winit_handler)
        } else {
            None
        };

        // Assume that the default cursor is visible at startup
        input_pipeline.set_type_processing_state(
            InputType::MouseMove,
            RouterProcessingState::from_boolean(false),
        )?;

        let supports_relative_mouse = input_pipeline.produces_type(InputType::MouseMove);

        // Create Shortcuts handler
        let pipeline_hook = Box::new(ClientPipelineHook::new_from_event_loop(
            self.eventloop_proxy.clone(),
        ));

        let state_reader = pipeline_hook.get_state_reader();
        input_pipeline.set_hook(pipeline_hook)?;

        // Create InputContext
        self.input_ctx = Some(InputContext {
            pipeline: input_pipeline,
            state_reader,
            supports_relative_mouse,
            winit_handler,
        });

        Ok(())
    }

    fn create_connect_config(&self) -> Result<ConnectConfig> {
        let connect_config = libclient_rs::ConnectConfig::default(
            &self.cli_args.streamer_ip,
            crate::STREAMER_HOST,
            crate::STREAMER_CERT,
        )?;

        if let Some(port) = self.cli_args.streamer_port {
            connect_config.set_streamer_port(port);
        }

        Ok(connect_config)
    }

    fn create_streaming_session_config(
        cli_args: &CliArgs,
        input_ctx: &Option<InputContext>,
    ) -> Result<StreamingSessionConfig> {
        let session_config = libclient_rs::StreamingSessionConfig::default(cli_args.protocol)?;

        if let Some(bitrate) = cli_args.bitrate {
            session_config.set_bitrate(bitrate);
        }

        let video_config = libclient_rs::VideoSessionConfig::default(cli_args.display_index)?;
        video_config.set_codec(cli_args.video_codec);
        video_config.set_yuv444(cli_args.yuv444);

        session_config.set_video(video_config);
        session_config.set_audio(cli_args.audio);

        if let Some(input_ctx) = input_ctx.as_ref() {
            let input_config = libclient_rs::InputSessionConfig::default(&input_ctx.pipeline)?;
            session_config.set_input(input_config);
        }

        Ok(session_config)
    }

    fn create_player_config(
        cli_args: &CliArgs,
        window: &winit::window::Window,
    ) -> Result<PlayerConfig> {
        let window_handle = match window.raw_window_handle() {
            #[cfg(target_os = "linux")]
            raw_window_handle::RawWindowHandle::Xlib(handle) => {
                WindowHandle::from_x11(handle.window)
            }
            #[cfg(target_os = "macos")]
            raw_window_handle::RawWindowHandle::AppKit(handle) => {
                WindowHandle::from_ns_view(handle.ns_view as u64)
            }
            #[cfg(target_os = "windows")]
            raw_window_handle::RawWindowHandle::Win32(handle) => {
                WindowHandle::from_hwnd(handle.hwnd as u64)
            }
            _ => {
                panic!("Platform not supported")
            }
        };

        let player_config = libclient_rs::PlayerConfig::default(Some(window_handle))?;
        player_config.set_software_decode(cli_args.software_decode);

        Ok(player_config)
    }

    fn connect(&mut self) -> Result<()> {
        // Create Input pipeline
        if self.cli_args.inputs {
            self.create_input_context()?;
            self.handle_focused()?;
        }

        // Create client
        let connect_config = self.create_connect_config()?;
        let client = Client::new(&connect_config)?;

        let client_proxy = Box::new(ClientEventProxy::new(self.eventloop_proxy.clone()));
        client.set_event_handler(client_proxy)?;

        client.connect()?;
        self.client = Some(client);

        Ok(())
    }

    fn mouse_button_filter_pressed(pkt: &Packet) -> bool {
        match MouseButton::from_packet(pkt) {
            Some(mouse_button) => !mouse_button.pressed,
            None => {
                warn!("Expected a MouseButton, got {:?}", pkt.get_type());
                false
            }
        }
    }

    fn handle_focused(&mut self) -> Result<()> {
        if !self.focused {
            self.input_ctx
                .as_ref()
                .ok_or(Error::NoPipeline)?
                .pipeline
                .flush_keys()?;

            self.grab_cursor(false);
        } else if self.window.fullscreen().is_some() {
            self.grab_cursor(true);
        }

        self.input_ctx
            .as_ref()
            .ok_or(Error::NoPipeline)?
            .pipeline
            .set_global_processing_state(self.focused)?;

        Ok(())
    }

    fn handle_cursor_inside_window(&mut self, inside: bool) -> Result<()> {
        self.cursor_inside_window = inside;

        let input_ctx = self.input_ctx.as_ref().ok_or(Error::NoPipeline)?;

        let types = [InputType::MouseMove, InputType::MouseWheel];
        for t in types {
            input_ctx
                .pipeline
                .set_type_processing_state(t, RouterProcessingState::from_boolean(inside))?;
        }

        if inside {
            input_ctx.pipeline.set_type_processing_state(
                InputType::MouseButton,
                RouterProcessingState::from_boolean(true),
            )?;
        } else {
            input_ctx.pipeline.set_type_processing_state(
                InputType::MouseButton,
                RouterProcessingState::from_callback(Box::new(Self::mouse_button_filter_pressed)),
            )?;
        }

        Ok(())
    }

    fn stop_client(&mut self) -> Result<()> {
        let client = self.client.as_mut().ok_or(Error::NoClient)?;
        client.disconnect()?;
        Ok(())
    }

    fn grab_cursor(&self, grab: bool) {
        // The cursor should be contained in the Rect where the video is displayed.
        // However, not all platforms support this feature, and some are only able
        // to contain within the whole window.
        // Try to contain in a Rect at first, and fallback in the whole window contain
        // if not supported.
        let ret = if grab {
            use winit::dpi::{PhysicalSize, Position, Size};

            let pos = Position::Physical(PhysicalPosition::new(
                self.video_layout.video_pos.x as i32,
                self.video_layout.video_pos.y as i32,
            ));

            let size = Size::Physical(PhysicalSize::new(
                self.video_layout.video_size.width,
                self.video_layout.video_size.height,
            ));

            self.window.set_cursor_rect(Some((pos, size)))
        } else {
            self.window.set_cursor_rect(None)
        };

        if ret.is_err() {
            let grab = if grab {
                CursorGrabMode::Confined
            } else {
                CursorGrabMode::None
            };

            let ret = self.window.set_cursor_grab(grab);
            if let Err(err) = ret {
                warn!("Fail during grab_cursor({:?}): {:?}", grab, err);
            }
        }
    }

    fn set_host_display_size(&mut self, display: &libclient_rs::Display) {
        self.display_id = Some(display.id);

        self.video_layout.host_size = RectSize {
            width: display.width as u32,
            height: display.height as u32,
        };

        self.video_layout.update_display_layout();
    }

    fn handle_event(
        &mut self,
        event: Event<UserEvent>,
        control_flow: &mut ControlFlow,
    ) -> Result<()> {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } => {
                if window_id == self.window.id() {
                    self.stop_client()?;
                }
            }
            Event::UserEvent(UserEvent::Start) => {
                self.connect()?;
            }
            Event::UserEvent(UserEvent::StopRequest) => {
                self.stop_client()?;
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                self.focused = focused;
                self.handle_focused()?;
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
                self.video_layout.window_size = RectSize {
                    width: size.width,
                    height: size.height,
                };

                self.video_layout.update_display_layout();

                self.grab_cursor(self.window.fullscreen().is_some());
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                // Cast to our internal representation
                let position: PhysicalPosition<i32> = PhysicalPosition {
                    x: position.x as _,
                    y: position.y as _,
                };

                // Avoid to send the same MouseMove multiple time
                if let Some(last_cursor_pos) = self.last_cursor_pos {
                    if last_cursor_pos.x == position.x && last_cursor_pos.y == position.y {
                        return Ok(());
                    }
                }

                self.last_cursor_pos = Some(position);

                // Project and send the position if an handler is defined
                let Some(display_id) = self.display_id else {
                    return Ok(());
                };

                let Some(input_ctx) = self.input_ctx.as_ref() else {
                    return Ok(());
                };

                if let Some(winit_handler) = input_ctx.winit_handler.as_ref() {
                    let projected_pos = self.video_layout.local_to_host(
                        Position {
                            x: position.x as _,
                            y: position.y as _,
                        },
                        input_ctx.state_reader.has_one_mouse_button_pressed(),
                    );

                    if let Some(projected_pos) = projected_pos {
                        winit_handler.send_mouse_position(
                            display_id,
                            projected_pos.x as i16,
                            projected_pos.y as i16,
                        );
                    }
                } else {
                    warn!("WindowEvent::CursorMoved discarded because no input handler is defined");
                }
            }
            Event::DeviceEvent { event, .. } => {
                let Some(input_ctx) = self.input_ctx.as_ref() else {
                    return Ok(());
                };

                if let Some(winit_handler) = input_ctx.winit_handler.as_ref() {
                    winit_handler.consume_device_event(&event);
                } else {
                    warn!("DeviceEvent discarded because no input handler is defined");
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        device_id,
                        input,
                        is_synthetic,
                    },
                ..
            } => {
                let Some(input_ctx) = self.input_ctx.as_ref() else {
                    return Ok(());
                };

                if let Some(winit_handler) = input_ctx.winit_handler.as_ref() {
                    winit_handler.consume_window_event(
                        &WindowEvent::KeyboardInput {
                            device_id,
                            input,
                            is_synthetic,
                        },
                        self.window.scale_factor(),
                    );
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        device_id,
                        state,
                        button,
                        ..
                    },
                ..
            } => {
                let Some(input_ctx) = self.input_ctx.as_ref() else {
                    return Ok(());
                };

                if let Some(winit_handler) = input_ctx.winit_handler.as_ref() {
                    winit_handler.consume_window_event(
                        &WindowEvent::MouseInput {
                            device_id,
                            state,
                            button,
                            modifiers: winit::event::ModifiersState::default(),
                        },
                        self.window.scale_factor(),
                    );
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        device_id,
                        delta,
                        phase,
                        ..
                    },
                ..
            } => {
                let Some(input_ctx) = self.input_ctx.as_ref() else {
                    return Ok(());
                };

                if let Some(winit_handler) = input_ctx.winit_handler.as_ref() {
                    winit_handler.consume_window_event(
                        &WindowEvent::MouseWheel {
                            device_id,
                            delta,
                            phase,
                            modifiers: winit::event::ModifiersState::default(),
                        },
                        self.window.scale_factor(),
                    );
                }
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::ControlPlaneConnected {
                displays,
            })) => {
                let display = if self.cli_args.display_index < displays.len() {
                    &displays[self.cli_args.display_index]
                } else {
                    warn!(
                        "Display {} is out of range, fallback to first screen",
                        self.cli_args.display_index
                    );
                    &displays[0]
                };

                self.set_host_display_size(display);

                let client = self.client.as_mut().ok_or(Error::NoClient)?;

                let player_config = Self::create_player_config(&self.cli_args, &self.window)?;
                let session_config =
                    Self::create_streaming_session_config(&self.cli_args, &self.input_ctx)?;

                client.start_streaming(&session_config, &player_config)?;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::ControlPlaneConnectFailed)) => {
                warn!("Control plane connect failed");
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::DataPlaneConnectFailed)) => {
                warn!("Data plane connect failed");
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::DataPlaneClosed)) => {
                warn!("Data plane closed");
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::StreamingStartFailed)) => {
                warn!("Streaming start failed");
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::StreamingStopped)) => {
                info!("Streaming stopped. Restart streaming");

                // Create Input pipeline
                if self.cli_args.inputs {
                    self.create_input_context()?;
                    self.handle_focused()?;
                }

                let client = self.client.as_mut().ok_or(Error::NoClient)?;

                let streaming_config =
                    Self::create_streaming_session_config(&self.cli_args, &self.input_ctx)?;
                let player_config = Self::create_player_config(&self.cli_args, &self.window)?;
                client.start_streaming(&streaming_config, &player_config)?;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::Stopped { reason })) => {
                info!("Stopped. Reason: {reason:?}");

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.stop()?;

                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::DisplayListUpdated {
                displays,
            })) => {
                log::info!("Display list updated");
                let Some(display_id) = self.display_id else {
                    return Ok(());
                };

                let Some(display) = displays.iter().find(|&d| d.id == display_id) else {
                    warn!("Used display not found in {displays:?}");
                    return Ok(());
                };

                self.video_layout.host_size = RectSize {
                    width: display.width as u32,
                    height: display.height as u32,
                };

                self.video_layout.update_display_layout();
                self.grab_cursor(self.window.fullscreen().is_some());
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::StreamerPipelineEvent {
                type_,
                stage,
                event,
            })) => {
                info!("Got StreamerPipelineEvent. Type: {type_:?} - Stage: {stage:?} - Event: {event:?}");
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::Fullscreen)) => {
                if self.window.fullscreen().is_some() {
                    debug!("Exit fullscreen");
                    self.window.set_fullscreen(None);

                    self.video_layout.window_size = self.window_size;
                    self.video_layout.update_display_layout();
                } else {
                    debug!("Enter fullscreen");

                    let monitor = self.window.current_monitor();

                    if let Some(monitor) = monitor.as_ref() {
                        let monitor_size = monitor.size();

                        self.video_layout.window_size = RectSize {
                            width: monitor_size.width,
                            height: monitor_size.height,
                        };

                        self.video_layout.update_display_layout()
                    }

                    self.window
                        .set_fullscreen(Some(window::Fullscreen::Borderless(monitor)));
                }
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::RestartStreaming)) => {
                info!("Restart streaming requested");

                self.input_ctx = None;

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.stop_streaming()?;
            }
            Event::UserEvent(UserEvent::SetCursor(cursor)) => {
                // MousePosition and MouseMove are always produced on client side.
                // But only one must be sent, depending of the cursor visibility:
                // - Cursor visible: Send absolute position
                // - Cursor not visible: Send relative moves
                let input_ctx = self.input_ctx.as_ref().ok_or(Error::NoPipeline)?;

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
                        let position = self.video_layout.host_to_local(Position {
                            x: cursor.x as u32,
                            y: cursor.y as u32,
                        });

                        // Store this position because it will be raised by winit after calling
                        // set_cursor_position. This is just a local update, not a real MouseMove
                        let physical_position =
                            PhysicalPosition::new(position.x as _, position.y as _);
                        self.last_cursor_pos = Some(physical_position);

                        self.window.set_cursor_position(physical_position)?;
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
                        self.grab_cursor(false);
                    }

                    if input_ctx.supports_relative_mouse {
                        input_ctx.pipeline.set_type_processing_state(
                            InputType::MousePosition,
                            RouterProcessingState::from_boolean(true),
                        )?;
                        input_ctx.pipeline.set_type_processing_state(
                            InputType::MouseMove,
                            RouterProcessingState::from_boolean(false),
                        )?;
                    }
                } else {
                    self.window.set_cursor_visible(false);
                    self.grab_cursor(true);

                    if input_ctx.supports_relative_mouse {
                        input_ctx.pipeline.set_type_processing_state(
                            InputType::MousePosition,
                            RouterProcessingState::from_boolean(false),
                        )?;
                        input_ctx.pipeline.set_type_processing_state(
                            InputType::MouseMove,
                            RouterProcessingState::from_boolean(true),
                        )?;
                    }
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
    state: RefCell<State>,
}

impl Window {
    pub fn new(event_loop: EventLoop<UserEvent>, cli_args: CliArgs) -> Result<Self> {
        // Create Loop + Window
        let window_width: u32 = 800;
        let window_height: u32 = 600;

        let window = WindowBuilder::new()
            .with_title("Kyber Client")
            .with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height))
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

        video_layout.update_display_layout();

        let eventloop_proxy = event_loop.create_proxy();

        Ok(Self {
            event_loop,
            state: RefCell::new(State {
                eventloop_proxy,
                cli_args,
                client: None,
                window,
                window_size: RectSize {
                    width: window_width,
                    height: window_height,
                },
                focused: false,
                video_layout,
                display_id: None,
                input_ctx: None,
                last_cursor_pos: None,
                cursor_inside_window: false,
                cursor_visible: true,
            }),
        })
    }

    pub fn create_stop_request_cb(&mut self) -> StopRequestCb {
        let proxy = self.event_loop.create_proxy();

        Box::new(move || {
            proxy.send_event(UserEvent::StopRequest).unwrap();
        })
    }

    pub fn run_loop(self) -> Result<()> {
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            let mut state = self.state.borrow_mut();
            if let Err(err) = state.handle_event(event, control_flow) {
                warn!("Event handler failed during processing: {:?}", err);
            }
        });
    }
}
