// Project Kyber: main.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0-or-later
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

#![cfg_attr(feature = "win32-hide-console", windows_subsystem = "windows")]

mod client_proxy;
mod error;
mod event_loop;
mod icon;
mod inputs;
mod keyboard_grabber;
mod metrics;
mod oidc_manager;
mod parse_utils;
mod window;

use log::{error, info, warn, LevelFilter};
use log4ky::notice;
use winit::event_loop::EventLoopBuilder;

use kyclient_rs::{
    AuthCredentials, KymuxAudioProtocol, KymuxVideoProtocol, StreamingProtocol, VideoCodec,
};

use error::{Error, Result};
use event_loop::UserEvent;
use parse_utils::parse_bitrate;

pub enum DisplayParams {
    Single(usize),
    Multiple(usize),
}

enum TlsVerification {
    System,
    RootCert(String),
    Fingerprint(String),
    Tofu,
    TofuInteractive,
    KnownHosts,
    Skip,
}

struct TlsConfig {
    host: Option<String>,
    verification: TlsVerification,
}

pub struct CliArgs {
    streamer_ip: String,
    streamer_port: Option<u16>,

    tls_host: Option<String>,
    tls_verification: TlsVerification,

    auth_credentials: AuthCredentials,
    protocol: StreamingProtocol,
    video_codec: VideoCodec,
    bitrate: Option<u32>,
    display_params: DisplayParams,
    yuv444: bool,
    audio: bool,
    audio_buffer: Option<u32>, // ms, None means default value
    video_buffer: u32,
    inputs: bool,
    gamepad_force_xbox360: Option<bool>,
    software_decode: bool,
    intra_refresh: Option<bool>,
    clipboard: Option<bool>,
    keyboard_grab: bool,
    metrics: bool,
    auto_reconnect: bool,
}

impl CliArgs {
    fn create_connect_config(&self) -> Result<kyclient_rs::ConnectConfig> {
        let connect_config =
            kyclient_rs::ConnectConfig::default(&self.streamer_ip, &self.auth_credentials)?;

        if let Some(tls_host) = &self.tls_host {
            connect_config.set_tls_host(tls_host)?;
        }

        event_loop::configure_tls_verification(&connect_config, &self.tls_verification)?;

        if let Some(port) = self.streamer_port {
            connect_config.set_streamer_port(port);
        }

        connect_config.set_auto_reconnection(self.auto_reconnect);

        Ok(connect_config)
    }
}

fn load_tls_conf(matches: &clap::ArgMatches) -> Result<TlsConfig> {
    let tls_host: Option<String> = matches.get_one("tls-host").cloned();
    let tls_root_cert: Option<String> = matches.get_one("tls-root-cert").cloned();

    // Handle fingerprint: either from --tls-fingerprint or --tls-fingerprint-file
    let tls_fingerprint: Option<String> =
        if let Some(fp) = matches.get_one::<String>("tls-fingerprint") {
            Some(fp.clone())
        } else if let Some(file_path) = matches.get_one::<String>("tls-fingerprint-file") {
            let content = std::fs::read_to_string(file_path).map_err(|e| {
                Error::Config(format!(
                    "Failed to read fingerprint file '{}': {}",
                    file_path, e
                ))
            })?;
            Some(content.trim().to_string())
        } else {
            None
        };

    let tls_tofu = matches.is_present("tls-tofu");
    let tls_tofu_interactive = matches.is_present("tls-tofu-interactive");
    let tls_known_hosts = matches.is_present("tls-known-hosts");
    let tls_skip_verification = matches.is_present("tls-skip-verification");

    // Count how many verification modes are specified
    let mode_count = [
        tls_root_cert.is_some(),
        tls_fingerprint.is_some(),
        tls_tofu,
        tls_tofu_interactive,
        tls_known_hosts,
        tls_skip_verification,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    if mode_count > 1 {
        return Err(Error::Config(
            "Only one TLS verification mode can be specified at a time".to_string(),
        ));
    }

    let verification = if tls_skip_verification {
        TlsVerification::Skip
    } else if let Some(fingerprint) = tls_fingerprint {
        TlsVerification::Fingerprint(fingerprint)
    } else if tls_tofu {
        TlsVerification::Tofu
    } else if tls_tofu_interactive {
        TlsVerification::TofuInteractive
    } else if tls_known_hosts {
        TlsVerification::KnownHosts
    } else if let Some(root_cert) = tls_root_cert {
        TlsVerification::RootCert(root_cert)
    } else {
        TlsVerification::System
    };

    Ok(TlsConfig {
        host: tls_host,
        verification,
    })
}

fn build_cli() -> clap::App<'static> {
    clap::App::new("Streaming client")
        .version(env!("VERGEN_GIT_DESCRIBE"))
        .about("IP of the streamer to connect to")
        .arg(
            clap::Arg::with_name("protocol")
                .long("protocol")
                .help("Protocol to use")
                .value_parser(["rtp", "kymux"])
                .default_value("kymux"),
        )
        .arg(
            clap::Arg::with_name("kymux-video")
                .long("kymux-video")
                .help("Kymux video protocol to use")
                .value_parser(["reliable", "gopstream", "unreliable", "unreliable_fec"])
                .default_value("reliable"),
        )
        .arg(
            clap::Arg::with_name("kymux-audio")
                .long("kymux-audio")
                .help("Kymux audio protocol to use")
                .value_parser(["reliable", "unreliable", "unreliable_fec"])
                .default_value("reliable"),
        )
        .arg(
            clap::Arg::with_name("video-codec")
                .long("video-codec")
                .help("Video codec to use")
                .value_parser(["h264", "hevc", "av1"])
                .default_value("h264"),
        )
        .arg(
            clap::Arg::with_name("444")
                .long("444")
                .help("Enable YUV444")
                .takes_value(false),
        )
        .arg(
            clap::Arg::with_name("bitrate")
                .long("bitrate")
                .help("Set connection bitrate (Unit: bytes)")
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("audio")
                .long("audio")
                .help("Enable audio")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("audio-buffer")
                .long("audio-buffer")
                .help("Set audio buffer (in ms)")
                .value_parser(clap::value_parser!(u32))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("video-buffer")
                .long("video-buffer")
                .help("Set video buffer (in ms)")
                .value_parser(clap::value_parser!(u32))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("inputs")
                .long("inputs")
                .help("Enable inputs")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("port")
                .long("port")
                .help("Port to use for connection")
                .value_parser(clap::value_parser!(u16))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("display-idx")
                .long("display-idx")
                .help("Display Id to access")
                .value_parser(clap::value_parser!(usize))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("display-count")
                .long("display-count")
                .help("Display count to stream")
                .value_parser(clap::value_parser!(usize))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("software-decode")
                .long("software-decode")
                .help("Enable video software decoding")
                .takes_value(false),
        )
        .arg(
            clap::Arg::with_name("intra-refresh")
                .long("intra-refresh")
                .help("Enable Intra refresh (is supported by the encoder)")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("gamepad-force-xbox360")
                .long("gamepad-force-xbox360")
                .help("Force to plug gamepads as Xbox 360 controllers")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("clipboard")
                .long("clipboard")
                .help("Enable Clipboard support")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("keyboard-grab")
                .long("keyboard-grab")
                .help("Enable keyboard grabbing for immersive mode. Default: true")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("tls-host")
                .long("tls-host")
                .help("TLS hostname to give during TLS connection")
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("tls-root-cert")
                .long("tls-root-cert")
                .help("TLS root certificate to trust")
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("tls-fingerprint")
                .long("tls-fingerprint")
                .help("TLS certificate fingerprint to verify (format: sha256:XX:XX:...)")
                .takes_value(true)
                .conflicts_with_all(&[
                    "tls-fingerprint-file",
                    "tls-tofu",
                    "tls-tofu-interactive",
                    "tls-known-hosts",
                    "tls-skip-verification",
                ]),
        )
        .arg(
            clap::Arg::with_name("tls-fingerprint-file")
                .long("tls-fingerprint-file")
                .help("Read TLS fingerprint from file (avoids shell history exposure)")
                .takes_value(true)
                .value_name("FILE")
                .conflicts_with_all(&[
                    "tls-fingerprint",
                    "tls-tofu",
                    "tls-tofu-interactive",
                    "tls-known-hosts",
                    "tls-skip-verification",
                ]),
        )
        .arg(
            clap::Arg::with_name("tls-tofu")
                .long("tls-tofu")
                .help(
                    "Enable Trust-On-First-Use verification (auto-accepts first connection). \
                       Known hosts stored in: Linux: ~/.local/share/kyber/known_hosts, \
                       Windows: %LOCALAPPDATA%\\kyber\\known_hosts",
                )
                .takes_value(false)
                .conflicts_with_all(&[
                    "tls-tofu-interactive",
                    "tls-known-hosts",
                    "tls-skip-verification",
                ]),
        )
        .arg(
            clap::Arg::with_name("tls-tofu-interactive")
                .long("tls-tofu-interactive")
                .help("Enable interactive TOFU (prompts yes/no for unknown hosts, CLI only)")
                .takes_value(false)
                .conflicts_with_all(&["tls-tofu", "tls-known-hosts", "tls-skip-verification"]),
        )
        .arg(
            clap::Arg::with_name("tls-known-hosts")
                .long("tls-known-hosts")
                .help("Strict mode: only accept hosts already in known_hosts (rejects unknown)")
                .takes_value(false)
                .conflicts_with_all(&["tls-tofu", "tls-tofu-interactive", "tls-skip-verification"]),
        )
        .arg(
            clap::Arg::with_name("tls-tofu-accept")
                .long("tls-tofu-accept")
                .help("Add a host to known_hosts: --tls-tofu-accept <hostname> <fingerprint>")
                .takes_value(true)
                .number_of_values(2)
                .value_names(&["HOSTNAME", "FINGERPRINT"]),
        )
        .arg(
            clap::Arg::with_name("tls-tofu-remove")
                .long("tls-tofu-remove")
                .help("Remove a host from known_hosts")
                .takes_value(true)
                .value_name("HOSTNAME"),
        )
        .arg(
            clap::Arg::with_name("tls-tofu-list")
                .long("tls-tofu-list")
                .help("List all hosts in known_hosts and exit")
                .takes_value(false),
        )
        .arg(
            clap::Arg::with_name("tls-skip-verification")
                .long("tls-skip-verification")
                .help("DANGER: Skip TLS certificate verification (testing only)")
                .takes_value(false),
        )
        .arg(
            clap::Arg::with_name("metrics")
                .long("metrics")
                .help("Enable metrics")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("auto-reconnect")
                .long("auto-reconnect")
                .help("Enable automatic reconnection on connection loss. Default: true")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .next_help_heading("Authentication")
        .arg(
            clap::Arg::new("username")
                .long("auth-username")
                .help("Basic authentication username")
                .default_value("Default user"),
        )
        .arg(
            clap::Arg::new("password")
                .long("auth-password")
                .help("Basic authentication password")
                .default_value("Default password"),
        )
        .arg(
            clap::Arg::new("jwt")
                .long("auth-token")
                .help("JsonWebToken authentication token")
                .takes_value(true),
        )
        .arg(
            clap::Arg::new("oidc")
                .long("auth-oidc")
                .help("Use OIDC authentication (server-initiated challenge)")
                .takes_value(false)
                .conflicts_with("jwt"),
        )
        .group(
            clap::ArgGroup::new("basic_auth")
                .args(&["username", "password"])
                .requires_all(&["username", "password"])
                .multiple(true)
                .conflicts_with_all(&["jwt", "oidc"]),
        )
        .arg(clap::Arg::with_name("STREAMER_IP").index(1))
        .arg(
            clap::Arg::with_name("log-level")
                .long("log-level")
                .help("Log level: trace, debug, info, warn, error, off. Overrides the KYBER_LOG_LEVEL root level.")
                .takes_value(true)
                .value_name("LEVEL")
                .value_parser(clap::builder::PossibleValuesParser::new([
                    "off", "error", "warn", "info", "debug", "trace",
                ])),
        )
}

fn main() {
    let matches = build_cli().get_matches();

    let vlc_level = std::env::var("KYBER_VLC_VERBOSE")
        .is_ok()
        .then_some(LevelFilter::Debug);
    let cli_level: Option<LevelFilter> = matches
        .value_of("log-level")
        .map(|s| s.parse().expect("level validated by clap"));
    let Ok(_log_handle) = log4ky::LoggerBuilder::new("kyclient")
        .base(log4ky::LogConfig {
            scope: log4ky::LogScope::User,
            ..Default::default()
        })
        .console(!cfg!(feature = "win32-hide-console"))
        .default_target_level("vlc", vlc_level)
        .cli_level(cli_level)
        .init()
    else {
        // Unreachable with no config file; invalid log config is fatal by contract.
        std::process::exit(1);
    };

    kynput_rs::init_logger();
    kyclient_rs::init();

    info!("Kyber client. Revision {}", env!("VERGEN_GIT_DESCRIBE"));

    // Handle utility commands (these exit without connecting)
    if let Some(values) = matches.values_of("tls-tofu-accept") {
        let values: Vec<&str> = values.collect();
        if values.len() == 2 {
            let hostname = values[0];
            let fingerprint = values[1];
            match kyclient_rs::known_hosts_add(hostname, fingerprint) {
                Ok(()) => {
                    info!("Added host '{}' to known_hosts", hostname);
                    println!("Host '{}' added to known_hosts.", hostname);
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Failed to add host: {:?}", e);
                    notice!("Error: Failed to add host to known_hosts");
                    std::process::exit(1);
                }
            }
        } else {
            notice!(
                "Error: --tls-tofu-accept requires exactly 2 arguments: <hostname> <fingerprint>"
            );
            std::process::exit(1);
        }
    }

    if let Some(hostname) = matches.value_of("tls-tofu-remove") {
        match kyclient_rs::known_hosts_remove(hostname) {
            Ok(true) => {
                info!("Removed host '{}' from known_hosts", hostname);
                println!("Host '{}' removed from known_hosts.", hostname);
                std::process::exit(0);
            }
            Ok(false) => {
                warn!("Host '{}' not found in known_hosts", hostname);
                println!("Host '{}' not found in known_hosts.", hostname);
                std::process::exit(1);
            }
            Err(e) => {
                error!("Failed to remove host: {:?}", e);
                notice!("Error: Failed to remove host from known_hosts");
                std::process::exit(1);
            }
        }
    }

    if matches.is_present("tls-tofu-list") {
        match kyclient_rs::known_hosts_list() {
            Ok(hosts) => {
                if hosts.is_empty() {
                    println!("No hosts in known_hosts.");
                } else {
                    println!("Known hosts:");
                    for (hostname, fingerprint) in hosts {
                        println!("  {} {}", hostname, fingerprint);
                    }
                }
                std::process::exit(0);
            }
            Err(e) => {
                error!("Failed to list hosts: {:?}", e);
                notice!("Error: Failed to list known_hosts");
                std::process::exit(1);
            }
        }
    }

    // For normal connection, STREAMER_IP is required
    let Some(streamer_ip) = matches.value_of("STREAMER_IP") else {
        notice!(
            "Error: STREAMER_IP is required\n\
             Usage: kyclient <STREAMER_IP> [OPTIONS]"
        );
        return;
    };
    let streamer_ip = String::from(streamer_ip);
    let streamer_port: Option<u16> = matches.get_one("port").copied();

    let protocol = match matches.value_of("protocol") {
        Some("rtp") => StreamingProtocol::Rtp,
        Some("kymux") => {
            let video_protocol = match matches.value_of("kymux-video") {
                None | Some("reliable") => KymuxVideoProtocol::Reliable,
                Some("gopstream") => KymuxVideoProtocol::GopStream,
                Some("unreliable") => KymuxVideoProtocol::Unreliable,
                Some("unreliable_fec") => KymuxVideoProtocol::UnreliableFec,
                _ => {
                    error!("No valid kymux video protocol specified");
                    return;
                }
            };
            let audio_protocol = match matches.value_of("kymux-audio") {
                None | Some("reliable") => KymuxAudioProtocol::Reliable,
                Some("unreliable") => KymuxAudioProtocol::Unreliable,
                Some("unreliable_fec") => KymuxAudioProtocol::UnreliableFec,
                _ => {
                    error!("No valid kymux audio protocol specified");
                    return;
                }
            };
            StreamingProtocol::Kymux {
                video_protocol,
                audio_protocol,
            }
        }
        _ => {
            error!("No valid protocol specified");
            return;
        }
    };

    let video_codec = match matches.value_of("video-codec") {
        Some("h264") => VideoCodec::H264,
        Some("hevc") => VideoCodec::HEVC,
        Some("av1") => VideoCodec::AV1,
        _ => {
            error!("No valid video_codec specified");
            return;
        }
    };

    let bitrate = matches
        .value_of("bitrate")
        .map(|bitrate| parse_bitrate(bitrate.into()).unwrap());

    let display_index = matches.get_one("display-idx").copied();
    let display_count = matches.get_one("display-count").copied();

    let display_params = match (display_index, display_count) {
        (None, None) => DisplayParams::Single(0),
        (None, Some(1)) => DisplayParams::Single(0),
        (Some(display_index), None) => DisplayParams::Single(display_index),
        (None, Some(display_count)) => DisplayParams::Multiple(display_count),
        (Some(_), Some(_)) => {
            error!("display-idx and display-count are mutually exclusive");
            return;
        }
    };

    let auth_credentials = if matches.is_present("oidc") {
        AuthCredentials::Oidc {
            token: String::new(),
        }
    } else if let Some(jwt) = matches.get_one::<String>("jwt") {
        AuthCredentials::Jwt { token: jwt.clone() }
    } else {
        AuthCredentials::Basic {
            // Presence check is done by clap.
            username: matches.get_one::<String>("username").unwrap().clone(),
            password: matches.get_one::<String>("password").unwrap().clone(),
        }
    };

    let yuv444 = matches.is_present("444");
    let audio = matches.get_one("audio").copied().unwrap_or(true);
    let audio_buffer = matches.get_one("audio-buffer").copied();
    let inputs = matches.get_one("inputs").copied().unwrap_or(true);
    let gamepad_force_xbox360 = matches.get_one("gamepad-force-xbox360").copied();
    let software_decode = matches.is_present("software-decode");
    let intra_refresh = matches.get_one("intra-refresh").copied();
    let video_buffer = matches.get_one("video-buffer").copied().unwrap_or(0);
    let clipboard = matches.get_one("clipboard").copied();
    let keyboard_grab = matches.get_one("keyboard-grab").copied().unwrap_or(true);
    let metrics = matches.get_one("metrics").copied().unwrap_or(false);
    let auto_reconnect = matches.get_one("auto-reconnect").copied().unwrap_or(true);

    let tls_config = load_tls_conf(&matches).unwrap_or_else(|e| {
        error!("{}", e);
        notice!("Error: {}", e);
        std::process::exit(1);
    });

    let cli_args = CliArgs {
        streamer_ip: streamer_ip.clone(),
        streamer_port,
        tls_host: tls_config.host,
        tls_verification: tls_config.verification,
        auth_credentials,
        protocol,
        video_codec,
        bitrate,
        display_params,
        yuv444,
        audio,
        audio_buffer,
        video_buffer,
        inputs,
        gamepad_force_xbox360,
        software_decode,
        intra_refresh,
        clipboard,
        keyboard_grab,
        metrics,
        auto_reconnect,
    };

    let mut event_loop_builder = EventLoopBuilder::<UserEvent>::with_user_event();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        event_loop_builder.with_x11();
    }

    // Create window
    let event_loop = event_loop_builder.build();

    let icon = icon::load_icon().ok();

    let mut event_loop = event_loop::EventLoopWrapper::new(event_loop, cli_args, icon).unwrap();

    let stop_req_cb = event_loop.create_stop_request_cb();
    ctrlc::set_handler(stop_req_cb).unwrap();

    event_loop.run().unwrap();
}
