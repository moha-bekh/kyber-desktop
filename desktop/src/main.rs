mod client_proxy;
mod cursor_handler;
mod error;
mod input_handler;
mod parse_utils;
mod pipeline_hook;
mod video_layout;
mod window;

use log::{error, info, LevelFilter};
use winit::event_loop::EventLoopBuilder;

use libclient_rs::{KymuxVideoProtocol, StreamingProtocol, VideoCodec};

use parse_utils::parse_bitrate;
use pipeline_hook::ClientPipelineHook;
use window::UserEvent;

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

pub struct CliArgs {
    streamer_ip: String,
    streamer_port: Option<u16>,
    protocol: StreamingProtocol,
    video_codec: VideoCodec,
    bitrate: Option<u32>,
    display_index: usize,
    yuv444: bool,
    audio: bool,
    inputs: bool,
    software_decode: bool,
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
            clap::Arg::with_name("software_decode")
                .long("software_decode")
                .help("Enable video software decoding")
                .takes_value(false),
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

    let display_index = matches.get_one("display_idx").copied().unwrap_or(0);
    let yuv444 = matches.is_present("444");
    let audio = matches.get_one("audio").copied().unwrap_or(true);
    let inputs = matches.get_one("inputs").copied().unwrap_or(true);
    let software_decode = matches.is_present("software_decode");

    let cli_args = CliArgs {
        streamer_ip: streamer_ip.clone(),
        streamer_port,
        protocol,
        video_codec,
        bitrate,
        display_index,
        yuv444,
        audio,
        inputs,
        software_decode,
    };

    // Create window
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let eventloop_proxy = event_loop.create_proxy();

    let mut window = window::Window::new(event_loop, cli_args).unwrap();

    let stop_req_cb = window.create_stop_request_cb();
    ctrlc::set_handler(move || {
        stop_req_cb();
    })
    .unwrap();

    eventloop_proxy.send_event(UserEvent::Start).unwrap();

    window.run_loop().unwrap();
}
