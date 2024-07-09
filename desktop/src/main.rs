#![cfg_attr(feature = "win32-hide-console", windows_subsystem = "windows")]

mod client_proxy;
mod error;
mod event_loop;
mod icon;
mod inputs;
mod parse_utils;
mod window;

use log::{error, info, LevelFilter};
use winit::event_loop::EventLoopBuilder;

use libclient_rs::{KymuxVideoProtocol, StreamingProtocol, VideoCodec};

use error::{Error, Result};
use event_loop::UserEvent;
use parse_utils::parse_bitrate;

const STREAMER_HOST: &str = "kybertest";
const STREAMER_CERT: &str = "kybertest_rootca_cert.pem";

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
    protocol: StreamingProtocol,
    video_codec: VideoCodec,
    bitrate: Option<u32>,
    display_params: DisplayParams,
    yuv444: bool,
    audio: bool,
    inputs: bool,
    gamepad_force_xbox360: Option<bool>,
    software_decode: bool,
    intra_refresh: Option<bool>,
}

fn main() {
    let _log_handle = setup_logs("client", LevelFilter::Info);

    libkynput_rs::init_logger();
    libclient_rs::init_logger();

    info!("Kyber client. Revision {}", env!("VERGEN_GIT_DESCRIBE"));

    let matches = clap::App::new("Streaming client")
        .about("IP of the streamer to connect to")
        .arg(
            clap::Arg::with_name("protocol")
                .long("protocol")
                .help("Protocol to use")
                .value_parser([
                    "rtp",
                    "kymux",
                    "kymux_reliable",
                    "kymux_gopstream",
                    "kymux_unreliable",
                    "kymux_unreliable_fec",
                ])
                .default_value("kymux"),
        )
        .arg(
            clap::Arg::with_name("video_codec")
                .long("video_codec")
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
            clap::Arg::with_name("display_idx")
                .long("display_idx")
                .help("Display Id to access")
                .value_parser(clap::value_parser!(usize))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("display_count")
                .long("display_count")
                .help("Display count to stream")
                .value_parser(clap::value_parser!(usize))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("software_decode")
                .long("software_decode")
                .help("Enable video software decoding")
                .takes_value(false),
        )
        .arg(
            clap::Arg::with_name("intra_refresh")
                .long("intra_refresh")
                .help("Enable Intra refresh (is supported by the encoder)")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("gamepad_force_xbox360")
                .long("gamepad_force_xbox360")
                .help("Force to plug gamepads as Xbox 360 controllers")
                .value_parser(clap::value_parser!(bool))
                .takes_value(true),
        )
        .arg(clap::Arg::with_name("STREAMER_IP").required(true).index(1))
        .get_matches();

    let streamer_ip = String::from(matches.value_of("STREAMER_IP").unwrap());
    let streamer_port: Option<u16> = matches.get_one("port").copied();

    let protocol = match matches.value_of("protocol") {
        Some("rtp") => StreamingProtocol::Rtp,
        Some("kymux") | Some("kymux_reliable") => {
            StreamingProtocol::Kymux(KymuxVideoProtocol::Reliable)
        }
        Some("kymux_gopstream") => StreamingProtocol::Kymux(KymuxVideoProtocol::GopStream),
        Some("kymux_unreliable") => StreamingProtocol::Kymux(KymuxVideoProtocol::Unreliable),
        Some("kymux_unreliable_fec") => StreamingProtocol::Kymux(KymuxVideoProtocol::UnreliableFec),
        _ => {
            error!("No valid protocol specified");
            return;
        }
    };

    let video_codec = match matches.value_of("video_codec") {
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

    let display_index = matches.get_one("display_idx").copied();
    let display_count = matches.get_one("display_count").copied();

    let display_params = match (display_index, display_count) {
        (None, None) => DisplayParams::Single(0),
        (None, Some(1)) => DisplayParams::Single(0),
        (Some(display_index), None) => DisplayParams::Single(display_index),
        (None, Some(display_count)) => DisplayParams::Multiple(display_count),
        (Some(_), Some(_)) => {
            error!("display_idx and display_count are mutually exclusive");
            return;
        }
    };

    let yuv444 = matches.is_present("444");
    let audio = matches.get_one("audio").copied().unwrap_or(true);
    let inputs = matches.get_one("inputs").copied().unwrap_or(true);
    let gamepad_force_xbox360 = matches.get_one("gamepad_force_xbox360").copied();
    let software_decode = matches.is_present("software_decode");
    let intra_refresh = matches.get_one("intra_refresh").copied();

    let cli_args = CliArgs {
        streamer_ip: streamer_ip.clone(),
        streamer_port,
        protocol,
        video_codec,
        bitrate,
        display_params,
        yuv444,
        audio,
        inputs,
        gamepad_force_xbox360,
        software_decode,
        intra_refresh,
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
