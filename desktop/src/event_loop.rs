// Project Kyber: event_loop.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0
//
// This file is both under dual license: AGPLv3 and a Commercial one.
//
// ----
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::sync::Arc;

use log::{error, info, warn};
use raw_window_handle::RawWindowHandle;
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Icon, WindowBuilder},
};

use libclient_rs::{
    Client, ConnectConfig, StreamingSessionConfig, TofuPrompt, VideoPlayerConfig, WindowHandle,
};

use crate::error::{Error, Result};
use crate::inputs::{InputContext, ShortcutType};
use crate::metrics::MetricsProcessor;
use crate::window::{Position, Size, Window, WindowControlWinit, WindowsStateMachine};
use crate::{client_proxy::ClientEventProxy, DisplayParams};
use crate::{CliArgs, TlsVerification};

/// Print SSH-style warning when a host's certificate has changed.
fn print_cert_changed_warning(hostname: &str, expected: &str, actual: &str) {
    eprintln!();
    eprintln!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
    eprintln!("@    WARNING: REMOTE HOST CERTIFICATE HAS CHANGED!       @");
    eprintln!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
    eprintln!("IT IS POSSIBLE THAT SOMEONE IS DOING SOMETHING NASTY!");
    eprintln!();
    eprintln!("Host: {}", hostname);
    eprintln!("Expected: {}", expected);
    eprintln!("Got:      {}", actual);
    eprintln!();
    eprintln!("To remove the old fingerprint, run:");
    eprintln!("  client --tls-tofu-remove {}", hostname);
    eprintln!();
}

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

fn configure_tls_verification(
    connect_config: &ConnectConfig,
    verification: &TlsVerification,
) -> Result<()> {
    match verification {
        TlsVerification::System => {}
        TlsVerification::RootCert(root_cert) => {
            connect_config.set_tls_root_cert(root_cert)?;
        }
        TlsVerification::Fingerprint(fingerprint) => {
            connect_config.set_tls_cert_fingerprint(fingerprint)?;
        }
        TlsVerification::Tofu => {
            connect_config.enable_tofu(|prompt| match prompt {
                TofuPrompt::UnknownHost { hostname, .. } => {
                    info!("TOFU: Trusting new host '{}'", hostname);
                    true
                }
                TofuPrompt::CertificateChanged {
                    hostname,
                    expected_fingerprint,
                    actual_fingerprint,
                } => {
                    print_cert_changed_warning(
                        &hostname,
                        &expected_fingerprint,
                        &actual_fingerprint,
                    );
                    false
                }
            })?;
        }
        TlsVerification::TofuInteractive => {
            connect_config.enable_tofu(|prompt| match prompt {
                TofuPrompt::UnknownHost {
                    hostname,
                    fingerprint,
                } => {
                    eprintln!();
                    eprintln!(
                        "The authenticity of host '{}' can't be established.",
                        hostname
                    );
                    eprintln!("Certificate fingerprint is {}.", fingerprint);
                    eprint!("Are you sure you want to continue connecting (yes/no)? ");

                    use std::io::Write;
                    std::io::stderr().flush().ok();

                    let mut input = String::new();
                    if std::io::stdin().read_line(&mut input).is_err() {
                        return false;
                    }

                    let accepted = input.trim().to_lowercase() == "yes";
                    if accepted {
                        info!("User accepted host '{}'", hostname);
                    } else {
                        info!("User rejected host '{}'", hostname);
                    }
                    accepted
                }
                TofuPrompt::CertificateChanged {
                    hostname,
                    expected_fingerprint,
                    actual_fingerprint,
                } => {
                    print_cert_changed_warning(
                        &hostname,
                        &expected_fingerprint,
                        &actual_fingerprint,
                    );
                    false
                }
            })?;
        }
        TlsVerification::KnownHosts => {
            connect_config.enable_tofu(|prompt| match prompt {
                TofuPrompt::UnknownHost {
                    hostname,
                    fingerprint,
                } => {
                    eprintln!();
                    eprintln!("Host '{}' is not in known_hosts.", hostname);
                    eprintln!("Fingerprint: {}", fingerprint);
                    eprintln!();
                    eprintln!("To add this host, run:");
                    eprintln!("  client --tls-tofu-accept {} {}", hostname, fingerprint);
                    eprintln!();
                    false
                }
                TofuPrompt::CertificateChanged {
                    hostname,
                    expected_fingerprint,
                    actual_fingerprint,
                } => {
                    print_cert_changed_warning(
                        &hostname,
                        &expected_fingerprint,
                        &actual_fingerprint,
                    );
                    false
                }
            })?;
        }
        TlsVerification::Skip => {
            warn!("TLS verification disabled - connection is NOT secure!");
            connect_config.tls_skip_verification();
        }
    }
    Ok(())
}

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
    saved_streaming_config: Option<StreamingSessionConfig>,
}

impl State {
    fn create_connect_config(&self) -> Result<ConnectConfig> {
        let connect_config = libclient_rs::ConnectConfig::default(&self.cli_args.streamer_ip)?;

        if let Some(credentials) = &self.cli_args.auth_credentials {
            connect_config.set_credentials(credentials)?;
        }

        if let Some(tls_host) = &self.cli_args.tls_host {
            connect_config.set_tls_host(tls_host)?;
        }

        configure_tls_verification(&connect_config, &self.cli_args.tls_verification)?;

        if let Some(port) = self.cli_args.streamer_port {
            connect_config.set_streamer_port(port);
        }

        connect_config.set_auto_reconnection(true);

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

            if input_ctx.supports_clipboard {
                if let Some(clipboard) = self.cli_args.clipboard {
                    input_config.clipboard(clipboard);
                }
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
        player_config.set_video_buffer(cli_args.video_buffer);

        Ok(player_config)
    }

    fn connect(&mut self) -> Result<()> {
        let metrics = self.cli_args.metrics;

        // Create client
        let connect_config = self.create_connect_config()?;
        let client = Client::new(&connect_config, metrics)?;

        let client_proxy = Box::new(ClientEventProxy::new(self.eventloop_proxy.clone()));
        client.set_event_handler(client_proxy)?;

        if metrics {
            let mut metrics_processor = MetricsProcessor::new()?;
            client.set_metrics_callback(Some(Box::new(move |metric| {
                metrics_processor.on_metric(metric);
            })));
        }

        client.connect()?;
        self.client = Some(client);

        Ok(())
    }

    fn stop_client(&mut self) -> Result<()> {
        let client = self.client.as_mut().ok_or(Error::NoClient)?;
        client.disconnect()?;
        Ok(())
    }

    fn start_streaming(&mut self) -> Result<()> {
        // Recreate input pipeline (handlers are not restartable after stop)
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

        self.saved_streaming_config = Some(streaming_config);

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
                if let Err(e) = self.connect() {
                    error!("Failed to connect: {:?}", e);
                    *control_flow = ControlFlow::Exit;
                    return Err(e);
                }
            }
            Event::UserEvent(UserEvent::StopRequest) => {
                self.stop_client()?;
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Focused(focused),
                ..
            } => {
                self.windows.focused(window_id, event_loop, focused)?;
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
                self.start_streaming()?;
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
                warn!("Data plane closed - waiting for reconnection");
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::Reconnecting {
                attempt,
                max_attempts,
            })) => {
                info!("Reconnecting... (attempt {}/{})", attempt, max_attempts);
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::Reconnected {
                should_restart_streaming,
            })) => {
                info!("Successfully reconnected to server");

                if should_restart_streaming {
                    info!("Restarting streaming session after reconnection");
                    self.start_streaming()?;
                }
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::ReconnectionFailed)) => {
                warn!("Reconnection failed after all attempts - exiting");
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::StreamingStartFailed)) => {
                warn!("Streaming start failed");
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::StreamingStopped)) => {
                info!("Streaming stopped. Restart streaming");
                self.start_streaming()?;
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::InputServiceConnected)) => {
                info!("Input service connected");

                self.windows.sync_cursor_position();
            }
            Event::UserEvent(UserEvent::Client(libclient_rs::Event::Stopped { reason })) => {
                info!("Stopped. Reason: {reason:?}");

                let client = self.client.as_mut().ok_or(Error::NoClient)?;
                client.stop()?;

                // Only exit if user requested disconnect, otherwise reconnection will be attempted
                match reason {
                    libclient_rs::StopReason::Requested => {
                        info!("User requested disconnect - exiting");
                        *control_flow = ControlFlow::Exit;
                    }
                    libclient_rs::StopReason::ControlPlaneClosed => {
                        info!("Control plane closed - waiting for reconnection");
                    }
                    libclient_rs::StopReason::ControlPlaneIoError => {
                        info!("Control plane I/O error - waiting for reconnection");
                    }
                    libclient_rs::StopReason::ControlPlaneTimeout => {
                        info!("Control plane timeout - waiting for reconnection");
                    }
                    libclient_rs::StopReason::DecodeError => {
                        warn!("Decode error occurred - exiting");
                        *control_flow = ControlFlow::Exit;
                    }
                }
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
                self.windows.toggle_fullscreen(event_loop);
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
                windows: WindowsStateMachine::<WindowControlWinit>::new(cli_args.keyboard_grab),
                cli_args,
                client: None,
                input_ctx: None,
                icon,
                volume: 1.0,
                mute: false,
                bitrate: None,
                saved_streaming_config: None,
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
