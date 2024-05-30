use std::cell::RefCell;
use std::sync::Arc;

use log::{info, warn};
use raw_window_handle::RawWindowHandle;
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::WindowBuilder,
};

use libclient_rs::{
    Client, ConnectConfig, StreamingSessionConfig, VideoPlayerConfig, WindowHandle,
};

use crate::client_proxy::ClientEventProxy;
use crate::error::{Error, Result};
use crate::inputs::{InputContext, ShortcutType};
use crate::window::{Position, Size, Window, WindowControlWinit, WindowsStateMachine};
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

struct State {
    eventloop_proxy: EventLoopProxy<UserEvent>,
    cli_args: CliArgs,
    client: Option<Client>,
    input_ctx: Option<Arc<InputContext>>,
    windows: WindowsStateMachine<WindowControlWinit>,
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

    fn get_initial_window_position(
        monitor: &winit::monitor::MonitorHandle,
        window_size: LogicalSize<f64>,
    ) -> LogicalPosition<f64> {
        let monitor_pos = monitor.os_position();
        let monitor_size = monitor.os_size();

        let x_offset = (monitor_size.width - window_size.width) / 2.0;
        let y_offset = (monitor_size.height - window_size.height) / 2.0;

        winit::dpi::LogicalPosition {
            x: monitor_pos.x + x_offset,
            y: monitor_pos.y + y_offset,
        }
    }

    fn get_initial_window_size(display: &libclient_rs::Display) -> LogicalSize<f64> {
        const RATIO: f64 = 1.5;
        LogicalSize::new(display.width as f64 / RATIO, display.height as f64 / RATIO)
    }

    fn create_single_window(
        &mut self,
        event_loop: &EventLoopWindowTarget<UserEvent>,
        display: &libclient_rs::Display,
        monitor: Option<MonitorHandle>,
    ) -> Result<()> {
        let window_size = Self::get_initial_window_size(display);

        let window_position =
            monitor.map(|monitor| Self::get_initial_window_position(&monitor, window_size));

        let window_builder = WindowBuilder::new()
            .with_title("Kyber Client")
            .with_inner_size(window_size);

        let window_builder = if let Some(window_position) = window_position {
            window_builder.with_position(window_position)
        } else {
            window_builder
        };

        let window = window_builder.build(event_loop)?;
        let window = Window::new(WindowControlWinit::new(window), display)?;
        self.windows.set_window(window);

        Ok(())
    }

    fn create_streaming_session_config(&self) -> Result<StreamingSessionConfig> {
        let session_config = libclient_rs::StreamingSessionConfig::default(self.cli_args.protocol)?;

        if let Some(bitrate) = self.cli_args.bitrate {
            session_config.set_bitrate(bitrate);
        }

        if let Some(window) = self.windows.window().as_ref() {
            let player_config =
                Self::create_player_config(&self.cli_args, window.raw_window_handle())?;

            let video_config =
                libclient_rs::VideoSessionConfig::default(window.display_id(), player_config)?;
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
            let pipeline = libkynput_rs::Pipeline::new(pipeline_config);

            let input_ctx = Arc::new(InputContext::new(self.eventloop_proxy.clone(), pipeline)?);

            self.windows.set_input_ctx(Arc::downgrade(&input_ctx));
            self.input_ctx = Some(input_ctx);
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

    fn stop_client(&mut self) -> Result<()> {
        let client = self.client.as_mut().ok_or(Error::NoClient)?;
        client.disconnect()?;
        Ok(())
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
                self.windows.focused(focused)?;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorEntered { device_id: _ },
                ..
            } => {
                self.windows.cursor_entered(true)?;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorLeft { device_id: _ },
                ..
            } => {
                self.windows.cursor_entered(false)?;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::Resized {
                        os_size: Some(os_size),
                        ..
                    },
                ..
            } => {
                let size = Size {
                    width: os_size.width,
                    height: os_size.height,
                };

                self.windows.resized(size);
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { os_position, .. },
                ..
            } => {
                let position = Position {
                    x: os_position.x,
                    y: os_position.y,
                };

                self.windows.cursor_moved(position)?;
            }
            Event::DeviceEvent { event, .. } => {
                self.windows.device_event(event);
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
                self.windows.window_event(WindowEvent::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                });
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
                self.windows.window_event(WindowEvent::MouseInput {
                    device_id,
                    state,
                    button,
                    modifiers: winit::event::ModifiersState::default(),
                });
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
                self.windows.window_event(WindowEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                    modifiers: winit::event::ModifiersState::default(),
                });
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

                self.create_single_window(event_loop, display, event_loop.primary_monitor())?;

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
                    let pipeline = libkynput_rs::Pipeline::new(pipeline_config);

                    let input_ctx =
                        Arc::new(InputContext::new(self.eventloop_proxy.clone(), pipeline)?);

                    self.windows.set_input_ctx(Arc::downgrade(&input_ctx));
                    self.input_ctx = Some(input_ctx);
                }

                let streaming_config = self.create_streaming_session_config()?;

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.start_streaming(&streaming_config)?;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::InputServiceConnected)) => {
                info!("Input service connected");

                self.windows.sync_cursor_position();
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
                info!("Display list updated");
                self.windows.display_list_updated(&displays);
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::StreamerPipelineEvent {
                type_,
                stage,
                event,
            })) => {
                info!("Got StreamerPipelineEvent. Type: {type_:?} - Stage: {stage:?} - Event: {event:?}");
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::Fullscreen)) => {
                self.windows.toggle_fullscreen();
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::RestartStreaming)) => {
                info!("Restart streaming requested");

                self.input_ctx = None;

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.stop_streaming()?;
            }
            Event::UserEvent(UserEvent::SetCursor(cursor)) => {
                self.windows.set_cursor(cursor)?;
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
                windows: WindowsStateMachine::<WindowControlWinit>::new(),
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
