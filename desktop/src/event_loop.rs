use std::cell::RefCell;

use log::{debug, info, warn};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget},
    window::{self, CursorGrabMode, CursorRgba, WindowBuilder},
};

use libclient_rs::{
    Client, ConnectConfig, StreamingSessionConfig, VideoPlayerConfig, WindowHandle,
};
use libkynput_rs::{CursorData, MouseButton, Packet, Pipeline};

use crate::client_proxy::ClientEventProxy;
use crate::error::{Error, Result};
use crate::inputs::{InputContext, ShortcutType};
use crate::video_layout::{Position, RectSize, VideoLayout};
use crate::CliArgs;

#[derive(Clone, Debug)]
pub enum UserEvent {
    Start,
    StopRequest,
    Shortcut(ShortcutType),
    Client(libclient_rs::Event),
    SetCursor(libkynput_rs::Cursor),
}

pub type StopRequestCb = Box<dyn Fn() + Send>;

fn mouse_button_filter_pressed(pkt: &Packet) -> bool {
    match MouseButton::from_packet(pkt) {
        Some(mouse_button) => !mouse_button.pressed,
        None => {
            warn!("Expected a MouseButton, got {:?}", pkt.get_type());
            false
        }
    }
}

struct WindowWrapper {
    window: winit::window::Window,
    window_size: RectSize,
    focused: bool,
    video_layout: VideoLayout,
    display_id: u32,
    cursor_inside_window: bool,
    last_cursor_pos: Option<PhysicalPosition<i32>>,
}

impl WindowWrapper {
    fn new(
        event_loop: &EventLoopWindowTarget<UserEvent>,
        display: &libclient_rs::Display,
    ) -> Result<Self> {
        let window_width: u32 = 800;
        let window_height: u32 = 600;

        let window = WindowBuilder::new()
            .with_title("Kyber Client")
            .with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height))
            .build(event_loop)?;

        let mut video_layout = VideoLayout {
            window_size: RectSize {
                width: window_width,
                height: window_height,
            },
            host_size: RectSize {
                width: display.width as _,
                height: display.height as _,
            },
            video_size: RectSize {
                width: 0,
                height: 0,
            },
            video_pos: Position { x: 0, y: 0 },
        };

        video_layout.update_display_layout();

        Ok(Self {
            window,
            window_size: RectSize {
                width: window_width,
                height: window_height,
            },
            focused: false,
            video_layout,
            display_id: display.id,
            cursor_inside_window: false,
            last_cursor_pos: None,
        })
    }

    fn id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    fn grab_cursor(&self, grab: bool) {
        // The cursor should be contained in the Rect where the video is displayed.
        // However, not all platforms support this feature, and some are only able
        // to contain within the whole window.
        // Try to contain in a Rect at first, and fallback in the whole window contain
        // if not supported.
        let ret = if grab {
            use winit::dpi::{Position, Size};

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

    fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible);
    }

    fn set_cursor_position(&self, position: PhysicalPosition<i32>) -> Result<()> {
        self.window.set_cursor_position(position)?;

        Ok(())
    }

    fn set_cursor_rgba(&self, cursor: CursorRgba) {
        self.window.set_cursor_rgba(cursor);
    }

    fn resized(&mut self, size: PhysicalSize<u32>) {
        self.window_size.width = size.width;
        self.window_size.height = size.height;

        debug!("Window resized");
        self.video_layout.window_size = RectSize {
            width: size.width,
            height: size.height,
        };

        self.video_layout.update_display_layout();
    }

    fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.window_size.width,
            height: self.window_size.height,
        }
    }

    fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    fn set_host_display_size(&mut self, display: &libclient_rs::Display) {
        if display.id != self.display_id {
            warn!("Trying to update host size from another display");
            return;
        }

        self.video_layout.host_size = RectSize {
            width: display.width as u32,
            height: display.height as u32,
        };

        self.video_layout.update_display_layout();
    }

    fn is_fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
    }

    fn toggle_fullscreen(&mut self) {
        if self.is_fullscreen() {
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

    fn position_inside_video(&self, position: &PhysicalPosition<i32>) -> bool {
        let video_size = &self.video_layout.video_size;
        let video_pos = &self.video_layout.video_pos;

        position.x >= video_pos.x as i32
            && position.x < video_size.width as i32
            && video_pos.y as i32 >= 0
            && position.y < video_size.height as i32
    }

    fn local_to_host(&self, cursor_pos: Position, capture_cursor: bool) -> Option<Position> {
        self.video_layout.local_to_host(cursor_pos, capture_cursor)
    }

    fn host_to_local(&self, host_pos: Position) -> Position {
        self.video_layout.host_to_local(host_pos)
    }

    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }
}

struct State {
    eventloop_proxy: EventLoopProxy<UserEvent>,
    cli_args: CliArgs,
    client: Option<Client>,
    input_ctx: Option<InputContext>,
    cursor_visible: bool,
    window: Option<WindowWrapper>,
}

impl State {
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

    fn create_streaming_session_config(&self) -> Result<StreamingSessionConfig> {
        let session_config = libclient_rs::StreamingSessionConfig::default(self.cli_args.protocol)?;

        if let Some(bitrate) = self.cli_args.bitrate {
            session_config.set_bitrate(bitrate);
        }

        if let Some(window) = &self.window {
            let player_config =
                Self::create_player_config(&self.cli_args, window.raw_window_handle())?;

            let video_config =
                libclient_rs::VideoSessionConfig::default(window.display_id, player_config)?;
            video_config.set_codec(self.cli_args.video_codec);
            video_config.set_yuv444(self.cli_args.yuv444);

            session_config.add_video(video_config);
        } else {
            warn!("No valid DisplayId found");
        }

        session_config.set_audio(self.cli_args.audio);

        if let Some(input_ctx) = self.input_ctx.as_ref() {
            let input_config = libclient_rs::InputSessionConfig::default(&input_ctx.pipeline())?;
            session_config.set_input(input_config);
        }

        Ok(session_config)
    }

    fn create_player_config(
        cli_args: &CliArgs,
        window_handle: RawWindowHandle,
    ) -> Result<VideoPlayerConfig> {
        let window_handle = match window_handle {
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

        let player_config = libclient_rs::VideoPlayerConfig::default(Some(window_handle))?;
        player_config.set_software_decode(cli_args.software_decode);

        Ok(player_config)
    }

    fn connect(&mut self) -> Result<()> {
        // Create Input pipeline
        if self.cli_args.inputs {
            let pipeline_config = libkynput_rs::Config::default_client();
            let pipeline = Pipeline::new(pipeline_config);

            self.input_ctx = Some(InputContext::new(self.eventloop_proxy.clone(), pipeline)?);
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

    fn handle_focused(&mut self) -> Result<()> {
        let Some(window) = &self.window else {
            return Ok(());
        };

        if !window.focused {
            self.input_ctx
                .as_ref()
                .ok_or(Error::NoPipeline)?
                .flush_keys()?;

            window.grab_cursor(false);
        } else if window.is_fullscreen() {
            window.grab_cursor(true);
        }

        self.input_ctx
            .as_ref()
            .ok_or(Error::NoPipeline)?
            .set_global_processing_state(window.focused)?;

        Ok(())
    }

    fn stop_client(&mut self) -> Result<()> {
        let client = self.client.as_mut().ok_or(Error::NoClient)?;
        client.disconnect()?;
        Ok(())
    }

    fn send_position(
        &self,
        input_ctx: &InputContext,
        position: PhysicalPosition<i32>,
        capture_cursor: bool,
    ) -> Result<()> {
        let Some(window) = &self.window else {
            return Ok(());
        };

        let projected_pos = window.local_to_host(
            Position {
                x: position.x as u32,
                y: position.y as u32,
            },
            capture_cursor,
        );

        if let Some(projected_pos) = projected_pos {
            input_ctx.send_mouse_position(window.display_id, projected_pos);
        }

        Ok(())
    }

    fn sync_cursor_position(&self) {
        let Some(window) = &self.window else {
            return;
        };

        // Wrap host cursor to match the local cursor position.
        // If cursor was outside the video, assume the cursor was
        // on the center of the window.
        let position = window
            .last_cursor_pos
            .filter(|position| window.position_inside_video(position))
            .unwrap_or_else(|| {
                let window_size = window.size();

                PhysicalPosition {
                    x: window_size.width as i32 / 2,
                    y: window_size.height as i32 / 2,
                }
            });

        if let Some(input_ctx) = self.input_ctx.as_ref() {
            let ret = self.send_position(input_ctx, position, false);
            if let Err(err) = ret {
                warn!("Failed to sync host cursor position: {err:?}");
            }
        }
    }

    fn handle_event(
        &mut self,
        event: Event<UserEvent>,
        event_loop: &EventLoopWindowTarget<UserEvent>,
        control_flow: &mut ControlFlow,
    ) -> Result<()> {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                self.stop_client()?;
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
                if let Some(window) = self.window.as_mut() {
                    window.focused = focused;
                }

                self.handle_focused()?;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorEntered { device_id: _ },
                ..
            } => {
                if let Some(window) = self.window.as_mut() {
                    window.cursor_inside_window = true;
                }

                if let Some(input_ctx) = self.input_ctx.as_ref() {
                    input_ctx.cursor_inside_window(true)?;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CursorLeft { device_id: _ },
                ..
            } => {
                if let Some(window) = self.window.as_mut() {
                    window.cursor_inside_window = false;
                }

                if let Some(input_ctx) = self.input_ctx.as_ref() {
                    input_ctx.cursor_inside_window(false)?;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                if let Some(window) = self.window.as_mut() {
                    window.resized(size);
                    window.grab_cursor(window.is_fullscreen());
                }
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
                if let Some(window) = self.window.as_mut() {
                    if let Some(last_cursor_pos) = window.last_cursor_pos {
                        if last_cursor_pos.x == position.x && last_cursor_pos.y == position.y {
                            return Ok(());
                        }
                    }

                    window.last_cursor_pos = Some(position);
                }

                // Project and send the position if an handler is defined
                if let Some(input_ctx) = self.input_ctx.as_ref() {
                    self.send_position(
                        input_ctx,
                        position,
                        input_ctx.has_one_mouse_button_pressed(),
                    )?;
                };
            }
            Event::DeviceEvent { event, .. } => {
                if let Some(input_ctx) = self.input_ctx.as_ref() {
                    input_ctx.device_event(&event);
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
                let scale_factor = {
                    let Some(window) = &self.window else {
                        return Ok(());
                    };

                    window.scale_factor()
                };

                if let Some(input_ctx) = self.input_ctx.as_ref() {
                    input_ctx.window_event(
                        &WindowEvent::KeyboardInput {
                            device_id,
                            input,
                            is_synthetic,
                        },
                        scale_factor,
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
                let scale_factor = {
                    let Some(window) = &self.window else {
                        return Ok(());
                    };

                    window.scale_factor()
                };

                if let Some(input_ctx) = self.input_ctx.as_ref() {
                    input_ctx.window_event(
                        &WindowEvent::MouseInput {
                            device_id,
                            state,
                            button,
                            modifiers: winit::event::ModifiersState::default(),
                        },
                        scale_factor,
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
                let scale_factor = {
                    let Some(window) = &self.window else {
                        return Ok(());
                    };

                    window.scale_factor()
                };

                if let Some(input_ctx) = self.input_ctx.as_ref() {
                    input_ctx.window_event(
                        &WindowEvent::MouseWheel {
                            device_id,
                            delta,
                            phase,
                            modifiers: winit::event::ModifiersState::default(),
                        },
                        scale_factor,
                    );
                };
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

                self.window = Some(WindowWrapper::new(event_loop, display)?);

                let session_config = self.create_streaming_session_config()?;

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.start_streaming(&session_config)?;
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
                    let pipeline_config = libkynput_rs::Config::default_client();
                    let pipeline = Pipeline::new(pipeline_config);

                    self.input_ctx =
                        Some(InputContext::new(self.eventloop_proxy.clone(), pipeline)?);
                    self.handle_focused()?;
                }

                let streaming_config = self.create_streaming_session_config()?;

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.start_streaming(&streaming_config)?;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::InputServiceConnected)) => {
                info!("Input service connected");

                self.sync_cursor_position();
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

                let Some(window) = self.window.as_mut() else {
                    return Ok(());
                };

                let Some(display) = displays.iter().find(|&d| d.id == window.display_id) else {
                    warn!("Used display not found in {displays:?}");
                    return Ok(());
                };

                window.set_host_display_size(display);
                window.grab_cursor(window.is_fullscreen());
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::StreamerPipelineEvent {
                type_,
                stage,
                event,
            })) => {
                info!("Got StreamerPipelineEvent. Type: {type_:?} - Stage: {stage:?} - Event: {event:?}");
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::Fullscreen)) => {
                if let Some(window) = self.window.as_mut() {
                    window.toggle_fullscreen();
                }
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::RestartStreaming)) => {
                info!("Restart streaming requested");

                self.input_ctx = None;

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.stop_streaming()?;
            }
            Event::UserEvent(UserEvent::SetCursor(cursor)) => {
                let Some(window) = self.window.as_mut() else {
                    return Ok(());
                };

                // MousePosition and MouseMove are always produced on client side.
                // But only one must be sent, depending of the cursor visibility:
                // - Cursor visible: Send absolute position
                // - Cursor not visible: Send relative moves
                if cursor.visible {
                    // Cursor must be restored before updating its shape.
                    // Otherwise on Linux, the default system cursor will be shown.
                    window.set_cursor_visible(true);

                    // If cursor wasn't previously visible, it must be moved at the position
                    // indicated by the Host. There is a drift between client cursor position,
                    // and host cursor position when injecting relative mouse move.
                    //
                    // To fix this issue, the Host sends the cursor position if known,
                    // otherwise i32::MAX is sent. Then the client must move its own cursor to
                    // match the host cursor coordinates.
                    if !self.cursor_visible && cursor.x != i32::MAX && cursor.y != i32::MAX {
                        let position = window.host_to_local(Position {
                            x: cursor.x as u32,
                            y: cursor.y as u32,
                        });

                        // Store this position because it will be raised by winit after calling
                        // set_cursor_position. This is just a local update, not a real MouseMove
                        let physical_position =
                            PhysicalPosition::new(position.x as _, position.y as _);
                        window.last_cursor_pos = Some(physical_position);

                        window.set_cursor_position(physical_position)?;
                    }

                    match cursor.data {
                        CursorData::Raw(data) => {
                            window.set_cursor_rgba(CursorRgba {
                                xhot: cursor.xhot,
                                yhot: cursor.yhot,
                                width: cursor.width,
                                height: cursor.height,
                                data,
                            });
                        }
                        CursorData::Png(_) => {
                            warn!("Got a PNG cursor");
                        }
                        CursorData::None => {}
                    }

                    if !window.is_fullscreen() {
                        window.grab_cursor(false);
                    }

                    if let Some(input_ctx) = &self.input_ctx {
                        input_ctx.set_mouse_relative(false)?;
                    }
                } else {
                    window.set_cursor_visible(false);
                    window.grab_cursor(true);

                    if let Some(input_ctx) = &self.input_ctx {
                        input_ctx.set_mouse_relative(true)?;
                    }
                }

                self.cursor_visible = cursor.visible;
            }
            _ => {}
        }

        Ok(())
    }
}

pub struct EventLoopWrapper {
    event_loop: EventLoop<UserEvent>,
    state: RefCell<State>,
}

impl EventLoopWrapper {
    pub fn new(event_loop: EventLoop<UserEvent>, cli_args: CliArgs) -> Result<Self> {
        let eventloop_proxy = event_loop.create_proxy();

        Ok(Self {
            event_loop,
            state: RefCell::new(State {
                eventloop_proxy,
                cli_args,
                client: None,
                input_ctx: None,
                cursor_visible: true,
                window: None,
            }),
        })
    }

    pub fn create_stop_request_cb(&mut self) -> StopRequestCb {
        let proxy = self.event_loop.create_proxy();

        Box::new(move || {
            proxy.send_event(UserEvent::StopRequest).unwrap();
        })
    }

    pub fn run(self) -> Result<()> {
        self.event_loop.run(move |event, event_loop, control_flow| {
            *control_flow = ControlFlow::Wait;

            let mut state = self.state.borrow_mut();
            if let Err(err) = state.handle_event(event, event_loop, control_flow) {
                warn!("Event handler failed during processing: {:?}", err);
            }
        });
    }
}
