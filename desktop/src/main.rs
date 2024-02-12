mod client_proxy;
mod cursor_handler;
mod error;
mod input_handler;
mod parse_utils;
mod pipeline_hook;
mod video_layout;
mod window;

use std::sync::Arc;

use log::{error, info, LevelFilter};
use winit::event_loop::EventLoopBuilder;

use libclient_rs::{Client, KymuxVideoProtocol, StreamingProtocol, VideoCodec, WindowHandle};
use libkynput_rs::{InputType, Pipeline};

use parse_utils::parse_bitrate;
use pipeline_hook::ClientPipelineHook;
use window::{ClientConfig, UserEvent};

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
    let audio = matches.get_one("audio").copied().unwrap_or(false);
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

    let pipeline_hook = Box::new(ClientPipelineHook::new_from_event_loop(
        event_loop.create_proxy(),
    ));

    let mut window = window::Window::new(
        event_loop,
        pipeline_hook.get_state_reader(),
        cli_args.display_index,
    )
    .unwrap();

    let stop_req_cb = window.create_stop_request_cb();
    ctrlc::set_handler(move || {
        stop_req_cb();
    })
    .unwrap();

    // Create InputPipeline
    let pipeline_config = libkynput_rs::Config::default_client();
    let input_pipeline = Pipeline::new(pipeline_config);

    input_pipeline
        .plug_handler(window.create_cursor_handler().unwrap())
        .unwrap();

    let missing_types: Vec<InputType> = input_handler::WinitHandler::producable_types()
        .iter()
        .filter(|&&t| !input_pipeline.produces_type(t))
        .copied()
        .collect();
    if !missing_types.is_empty() {
        info!("Winit will be used to produce types {:?}", missing_types);
        let handler = Arc::new(
            input_handler::WinitHandler::new(input_pipeline.get_input_sink(), missing_types)
                .unwrap(),
        );

        input_pipeline
            .plug_handler(libkynput_rs::create_handler_instance(handler.clone()))
            .unwrap();
        window.set_input_handler(handler).unwrap();
    }

    // Create ConnectConfig
    let connect_config = libclient_rs::ConnectConfig::default(
        cli_args.streamer_ip.as_str(),
        STREAMER_HOST,
        STREAMER_CERT,
    )
    .unwrap();

    if let Some(port) = cli_args.streamer_port {
        connect_config.set_streamer_port(port);
    }

    // Create StreamSessionConfig
    let session_config = libclient_rs::StreamingSessionConfig::default(cli_args.protocol).unwrap();

    let video_config = libclient_rs::VideoSessionConfig::default(cli_args.display_index).unwrap();
    video_config.set_codec(cli_args.video_codec);
    video_config.set_yuv444(cli_args.yuv444);

    if let Some(bitrate) = cli_args.bitrate {
        session_config.set_bitrate(bitrate);
    }

    session_config.set_video(video_config);
    session_config.set_audio(cli_args.audio);

    if cli_args.inputs {
        let input_config = libclient_rs::InputSessionConfig::default(&input_pipeline).unwrap();
        session_config.set_input(input_config);
    }

    // Create PlayerConfig
    let window_handle = match window.window_handle() {
        #[cfg(target_os = "linux")]
        raw_window_handle::RawWindowHandle::Xlib(handle) => WindowHandle::from_x11(handle.window),
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

    let player_config = libclient_rs::PlayerConfig::default(Some(window_handle)).unwrap();
    player_config.set_software_decode(cli_args.software_decode);

    // Create client
    let client = Client::new(&connect_config).unwrap();
    window
        .set_client(
            client.clone(),
            ClientConfig {
                session_config,
                player_config,
            },
        )
        .unwrap();

    // Create Shortcuts handler
    input_pipeline.set_hook(pipeline_hook).unwrap();

    // Setup input routine
    // Window => InputRouter
    window.set_input_pipeline(input_pipeline).unwrap();

    client.connect().unwrap();

    // Process window loop
    window.run_loop().unwrap();
}
