use std::cell::RefCell;
use std::cmp::Ordering;
use std::sync::Arc;

use log::{info, warn};
use raw_window_handle::RawWindowHandle;
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Icon, WindowBuilder},
};

use libclient_rs::{
    Client, ConnectConfig, StreamingSessionConfig, VideoPlayerConfig, WindowHandle,
};

use crate::error::{Error, Result};
use crate::inputs::{InputContext, ShortcutType};
use crate::window::{Position, Size, Window, WindowControlWinit, WindowsStateMachine};
use crate::CliArgs;
use crate::{client_proxy::ClientEventProxy, DisplayParams};

#[derive(Clone, Debug)]
pub enum UserEvent {
    Start,
    StopRequest,
    Shortcut(ShortcutType),
    Client(libclient_rs::Event),
    SetCursor(libkynput_rs::Cursor),
    LeftButtonReleased,
}

pub(crate) trait EventProxy: Send + 'static {
    fn send_event(&self, event: UserEvent) -> Result<()>;
}

#[derive(Clone)]
struct EventProxyWinit {
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
}

impl EventProxyWinit {
    fn new(proxy: winit::event_loop::EventLoopProxy<UserEvent>) -> Self {
        Self { proxy }
    }
}

impl EventProxy for EventProxyWinit {
    fn send_event(&self, event: UserEvent) -> Result<()> {
        self.proxy.send_event(event)?;

        Ok(())
    }
}

pub type StopRequestCb = Box<dyn Fn() + Send>;

struct State {
    eventloop_proxy: EventLoopProxy<UserEvent>,
    cli_args: CliArgs,
    client: Option<Client>,
    input_ctx: Option<Arc<InputContext>>,
    icon: Option<Icon>,
    windows: WindowsStateMachine<WindowControlWinit>,
    volume: f32,
    mute: bool,
    bitrate: Option<u32>,
}

impl State {
    fn create_connect_config(&self) -> Result<ConnectConfig> {
        let connect_config = libclient_rs::ConnectConfig::default(&self.cli_args.streamer_ip)?;

        if let Some(tls_host) = &self.cli_args.tls_host {
            connect_config.set_tls_host(tls_host)?;
        }

        if let Some(tls_root_cert) = &self.cli_args.tls_root_cert {
            connect_config.set_tls_root_cert(tls_root_cert)?;
        }

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
            .with_window_icon(self.icon.clone())
            .with_inner_size(window_size);

        let window_builder = if let Some(window_position) = window_position {
            window_builder.with_position(window_position)
        } else {
            window_builder
        };

        let window = window_builder.build(event_loop)?;
        let window = Window::new(WindowControlWinit::new(window), display)?;
        self.windows.add_window(window);

        Ok(())
    }

    fn create_multiple_windows(
        &mut self,
        event_loop: &EventLoopWindowTarget<UserEvent>,
        host_displays: &[libclient_rs::Display],
        requested_display_count: usize,
    ) -> Result<()> {
        // Sort host displays
        let mut host_displays: Vec<_> = host_displays.to_vec();
        host_displays.sort_by(|a, b| {
            let y = a.y.partial_cmp(&b.y).unwrap();
            if let Ordering::Equal = y {
                a.x.partial_cmp(&b.x).unwrap()
            } else {
                y
            }
        });

        // Get and sort local displays
        let mut local_displays: Vec<_> = event_loop.available_monitors().collect();
        local_displays.sort_by(|a, b| {
            let y = a.position().y.partial_cmp(&b.position().y).unwrap();

            if let Ordering::Equal = y {
                a.position().x.partial_cmp(&b.position().x).unwrap()
            } else {
                y
            }
        });

        // Get the correct display count to get
        let display_count = [
            host_displays.len(),
            local_displays.len(),
            requested_display_count,
        ]
        .iter()
        .copied()
        .min()
        .expect("No display count found");

        // Create all the window and attach them to the displays
        for i in 0..display_count {
            let host_display = &host_displays[i];
            let local_display = &local_displays[i];

            self.create_single_window(event_loop, host_display, Some(local_display.clone()))?;
        }

        Ok(())
    }

    fn create_windows(
        &mut self,
        event_loop: &EventLoopWindowTarget<UserEvent>,
        displays: &[libclient_rs::Display],
    ) -> Result<()> {
        match &self.cli_args.display_params {
            DisplayParams::Single(display_idx) => {
                let display_idx = if *display_idx < displays.len() {
                    *display_idx
                } else {
                    warn!("Display {display_idx} is out of range, fallback to first screen",);

                    0
                };

                self.create_single_window(
                    event_loop,
                    &displays[display_idx],
                    event_loop.primary_monitor(),
                )?;
            }
            DisplayParams::Multiple(display_count) => {
                self.create_multiple_windows(event_loop, displays, *display_count)?;
            }
        }

        Ok(())
    }

    fn create_streaming_session_config(&mut self) -> Result<StreamingSessionConfig> {
        let session_config = libclient_rs::StreamingSessionConfig::default(self.cli_args.protocol)?;

        if let Some(bitrate) = self.bitrate {
            session_config.set_bitrate(bitrate);
        } else if let Some(bitrate) = self.cli_args.bitrate {
            session_config.set_bitrate(bitrate);
        }

        self.bitrate = Some(session_config.get_bitrate());

        for window in self.windows.iter_windows() {
            let player_config =
                Self::create_player_config(&self.cli_args, window.raw_window_handle())?;

            let video_config =
                libclient_rs::VideoSessionConfig::default(window.display_id(), player_config)?;
            video_config.set_codec(self.cli_args.video_codec);
            video_config.set_yuv444(self.cli_args.yuv444);

            if let Some(intra_refresh) = self.cli_args.intra_refresh {
                video_config.set_intra_refresh(intra_refresh);
            }

            session_config.add_video(video_config);
        }

        if self.cli_args.audio {
            let audio_config = libclient_rs::AudioSessionConfig::new_default()?;
            if let Some(audio_buffer) = self.cli_args.audio_buffer {
                audio_config.set_audio_buffer(audio_buffer);
            }
            session_config.set_audio(audio_config);
        }

        if let Some(input_ctx) = self.input_ctx.as_ref() {
            let input_config = libclient_rs::InputSessionConfig::default(&input_ctx.pipeline())?;

            if let Some(force) = self.cli_args.gamepad_force_xbox360 {
                input_config.gamepad_force_xbox360(force);
            }

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

            let eventloop_proxy = EventProxyWinit::new(self.eventloop_proxy.clone());
            let input_ctx = Arc::new(InputContext::new(eventloop_proxy, pipeline)?);

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
                window_id,
                event: WindowEvent::Focused(focused),
                ..
            } => {
                self.windows.focused(window_id, focused)?;
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CursorEntered { device_id: _ },
                ..
            } => {
                self.windows.cursor_entered(window_id, true)?;
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CursorLeft { device_id: _ },
                ..
            } => {
                self.windows.cursor_entered(window_id, false)?;
            }
            Event::WindowEvent {
                window_id,
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

                self.windows.resized(window_id, size);
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CursorMoved { os_position, .. },
                ..
            } => {
                let position = Position {
                    x: os_position.x,
                    y: os_position.y,
                };

                self.windows.cursor_moved(window_id, position)?;
            }
            Event::DeviceEvent { event, .. } => {
                self.windows.device_event(event);
            }
            Event::WindowEvent {
                window_id,
                event:
                    WindowEvent::KeyboardInput {
                        device_id,
                        input,
                        is_synthetic,
                    },
                ..
            } => {
                self.windows.window_event(
                    window_id,
                    WindowEvent::KeyboardInput {
                        device_id,
                        input,
                        is_synthetic,
                    },
                );
            }
            Event::WindowEvent {
                window_id,
                event:
                    WindowEvent::MouseInput {
                        device_id,
                        state,
                        button,
                        ..
                    },
                ..
            } => {
                self.windows.window_event(
                    window_id,
                    WindowEvent::MouseInput {
                        device_id,
                        state,
                        button,
                        modifiers: winit::event::ModifiersState::default(),
                    },
                );
            }
            Event::WindowEvent {
                window_id,
                event:
                    WindowEvent::MouseWheel {
                        device_id,
                        delta,
                        phase,
                        ..
                    },
                ..
            } => {
                self.windows.window_event(
                    window_id,
                    WindowEvent::MouseWheel {
                        device_id,
                        delta,
                        phase,
                        modifiers: winit::event::ModifiersState::default(),
                    },
                );
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::ControlPlaneConnected {
                displays,
            })) => {
                self.create_windows(event_loop, &displays)?;

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

                    let eventloop_proxy = EventProxyWinit::new(self.eventloop_proxy.clone());
                    let input_ctx = Arc::new(InputContext::new(eventloop_proxy, pipeline)?);

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
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::AudioVolumeChanged {
                volume,
            })) => {
                info!("Volume changed: {volume}");
                self.volume = volume;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::AudioMuteChanged { mute })) => {
                info!("Volume muted: {mute}");
                self.mute = mute;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::BitrateChanged {
                bitrate,
            })) => {
                info!("Bitrate changed: {bitrate}");
                self.bitrate = Some(bitrate);
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
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::VolumeUp)) => {
                let mut volume = self.volume + 0.1;
                if volume > 1.0 {
                    volume = 1.0;
                }

                if let Some(client) = &self.client {
                    client.audio_set_volume(volume).unwrap();
                }
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::VolumeDown)) => {
                let mut volume = self.volume - 0.1;
                if volume < 0.0 {
                    volume = 0.0;
                }

                if let Some(client) = &self.client {
                    client.audio_set_volume(volume).unwrap();
                }
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::BitrateUp)) => {
                let Some(mut bitrate) = self.bitrate else {
                    return Ok(());
                };

                bitrate += 5_000_000;
                if bitrate > 70_000_000 {
                    bitrate = 70_000_000;
                }

                if let Some(client) = &self.client {
                    client.set_bitrate(bitrate).unwrap();
                }
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::BitrateDown)) => {
                let Some(mut bitrate) = self.bitrate else {
                    return Ok(());
                };

                bitrate -= 5_000_000;
                if bitrate < 5_000_000 {
                    bitrate = 5_000_000;
                }

                if let Some(client) = &self.client {
                    client.set_bitrate(bitrate).unwrap();
                }
            }
            Event::UserEvent(UserEvent::Shortcut(ShortcutType::ToggleMute)) => {
                let mute = !self.mute;

                if let Some(client) = &self.client {
                    client.audio_set_mute(mute).unwrap();
                }
            }
            Event::UserEvent(UserEvent::SetCursor(cursor)) => {
                self.windows.set_cursor(cursor)?;
            }
            Event::UserEvent(UserEvent::LeftButtonReleased) => {
                self.windows.left_button_released();
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
    pub fn new(
        event_loop: EventLoop<UserEvent>,
        cli_args: CliArgs,
        icon: Option<Icon>,
    ) -> Result<Self> {
        let eventloop_proxy = event_loop.create_proxy();

        Ok(Self {
            event_loop,
            state: RefCell::new(State {
                eventloop_proxy,
                cli_args,
                client: None,
                input_ctx: None,
                icon,
                windows: WindowsStateMachine::<WindowControlWinit>::new(),
                volume: 1.0,
                mute: false,
                bitrate: None,
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
