#![cfg_attr(feature = "win32-hide-console", windows_subsystem = "windows")]

mod client_proxy;
mod error;
mod event_loop;
mod icon;
mod inputs;
mod keyboard_grabber;
mod metrics;
mod parse_utils;
mod window;

use log::{error, info, LevelFilter};
use winit::event_loop::EventLoopBuilder;

use libclient_rs::{
    AuthCredentials, KymuxAudioProtocol, KymuxVideoProtocol, StreamingProtocol, VideoCodec,
};

use error::{Error, Result};
use event_loop::UserEvent;
use parse_utils::parse_bitrate;

fn setup_logs(logfile: &str, level: LevelFilter) -> log4rs::Handle {
    use std::path::Path;

    use log4rs::{
        append::{console::ConsoleAppender, file::FileAppender},
        config::{Appender, Config, Logger, Root},
        encode::pattern::PatternEncoder,
    };

    // Setup pattern
    let pattern_encoder = PatternEncoder::new("{d(%Y-%m-%dT%H:%M:%S.%3f)} {h({l})} {t} - {m}{n}");

    // Setup Console output
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(pattern_encoder.clone()))
        .build();

    // Compute log file
    let log_folder = "log";
    let log_file = format!("{logfile}.log");

    let log_path = if cfg!(windows) {
        let mut exe_path = std::env::current_exe().unwrap();
        exe_path.pop();

        Path::new(&exe_path).join(log_folder).join(log_file)
    } else {
        Path::new(log_folder).join(log_file)
    };

    // Setup File output
    let file = FileAppender::builder()
        .encoder(Box::new(pattern_encoder))
        .build(log_path)
        .unwrap();

    // Configure logger
    let mut config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)));

    if std::env::var("KYBER_VLC_VERBOSE").is_ok() {
        config = config.logger(Logger::builder().build("vlc", LevelFilter::Debug));
    }

    let config = config
        .build(
            Root::builder()
                .appender("file")
                .appender("stdout")
                .build(level),
        )
        .unwrap();

    log4rs::init_config(config).unwrap()
}

pub enum DisplayParams {
    Single(usize),
    Multiple(usize),
}

pub struct CliArgs {
    streamer_ip: String,
    streamer_port: Option<u16>,

    tls_host: Option<String>,
    tls_root_cert: Option<String>,

    auth_credentials: Option<AuthCredentials>,
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
    metrics: bool,
}

#[cfg(feature = "test-tls-cert")]
fn tls_get_test_params(
    matches: &clap::ArgMatches,
    tls_host: Option<String>,
    tls_root_cert: Option<String>,
) -> (Option<String>, Option<String>) {
    if !matches
        .get_one("tls-enable-test-cert")
        .copied()
        .unwrap_or(true)
    {
        return (tls_host, tls_root_cert);
    }

    const STREAMER_HOST: &str = "kybertest";
    const STREAMER_CERT: &str = "kybertest_rootca_cert.pem";

    let tls_host = tls_host.unwrap_or(STREAMER_HOST.into());
    let tls_root_cert = tls_root_cert.unwrap_or(STREAMER_CERT.into());

    (Some(tls_host), Some(tls_root_cert))
}

fn load_tls_conf(matches: &clap::ArgMatches) -> (Option<String>, Option<String>) {
    let tls_host: Option<String> = matches.get_one("tls-host").cloned();
    let tls_root_cert: Option<String> = matches.get_one("tls-root-cert").cloned();

    #[cfg(feature = "test-tls-cert")]
    let (tls_host, tls_root_cert) = tls_get_test_params(matches, tls_host, tls_root_cert);

    (tls_host, tls_root_cert)
}

fn main() {
    let _log_handle = setup_logs("client", LevelFilter::Info);

    libkynput_rs::init_logger();
    libclient_rs::init();

    info!("Kyber client. Revision {}", env!("VERGEN_GIT_DESCRIBE"));

    let app = clap::App::new("Streaming client")
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
            clap::Arg::with_name("metrics")
                .long("metrics")
                .help("Enable metrics")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("auth-type")
                .long("auth-type")
                .help("Authentication method")
                .value_parser(["basic", "jwt"])
                .default_value("basic"),
        )
        .arg(
            clap::Arg::with_name("auth-creds")
                .long("auth-credentials")
                .help("Authentication credentials")
                .default_value("anonymous"),
        )
        .arg(clap::Arg::with_name("STREAMER_IP").required(true).index(1));

    #[cfg(feature = "test-tls-cert")]
    let app = app.arg(
        clap::Arg::with_name("tls-enable-test-cert")
            .long("tls-enable-test-cert")
            .help("Enable TLS test certificates")
            .value_parser(clap::value_parser!(bool))
            .takes_value(true),
    );

    let matches = app.get_matches();

    let streamer_ip = String::from(matches.value_of("STREAMER_IP").unwrap());
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

    let creds = matches.value_of("auth-creds").unwrap().to_owned();
    let auth_credentials = match matches.value_of("auth-type") {
        Some("basic") => Some(AuthCredentials::Basic { username: creds }),
        Some("jwt") => Some(AuthCredentials::Jwt { token: creds }),
        None | Some(_) => unreachable!(),
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
    let metrics = matches.get_one("metrics").copied().unwrap_or(false);

    let (tls_host, tls_root_cert) = load_tls_conf(&matches);

    let cli_args = CliArgs {
        streamer_ip: streamer_ip.clone(),
        streamer_port,
        tls_host,
        tls_root_cert,
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
        metrics,
    };

    // Create window
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let eventloop_proxy = event_loop.create_proxy();

    let icon = icon::load_icon().ok();

    let mut event_loop = event_loop::EventLoopWrapper::new(event_loop, cli_args, icon).unwrap();

    let stop_req_cb = event_loop.create_stop_request_cb();
    ctrlc::set_handler(move || {
        stop_req_cb();
    })
    .unwrap();

    eventloop_proxy.send_event(UserEvent::Start).unwrap();

    event_loop.run().unwrap();
}
