mod client_proxy;
mod cursor_handler;
mod error;
mod input_handler;
mod pipeline_hook;
mod video_layout;
mod window;

use std::sync::Arc;

use log::{error, info, LevelFilter};
use winit::event_loop::EventLoopBuilder;

use libclient_rs::{Client, StreamingProtocol, VideoCodec, WindowHandle};
use libkynput_rs::{InputType, Pipeline};

use pipeline_hook::ClientPipelineHook;
use window::UserEvent;

const STREAMER_HOST: &str = "kybertest";
const STREAMER_CERT: &str = "kybertest_cert.pem";

fn main() {
    let filter_level = LevelFilter::Info;
    env_logger::Builder::new()
        .filter_level(filter_level)
        .format_timestamp_millis()
        .init();

    libkynput_rs::init_logger();
    libclient_rs::init_logger();

    println!("Kyber client. Revision {}", env!("VERGEN_GIT_DESCRIBE"));

    let matches = clap::App::new("Streaming client")
        .about("IP of the streamer to connect to")
        .arg(
            clap::Arg::with_name("FULLSCREEN")
                .short('f')
                .long("fullscreen")
                .help("Initial fullscreen state")
                .takes_value(false),
        )
        .arg(
            clap::Arg::with_name("protocol")
                .long("protocol")
                .help("Protocol to use")
                .value_parser(["rtp", "kymux"])
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
        .arg(clap::Arg::with_name("STREAMER_IP").required(true).index(1))
        .get_matches();

    let streamer_ip = String::from(matches.value_of("STREAMER_IP").unwrap());

    let protocol = match matches.value_of("protocol") {
        Some("rtp") => StreamingProtocol::Rtp,
        Some("kymux") => StreamingProtocol::Kymux,
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
            error!("No valid protocol specified");
            return;
        }
    };

    let yuv444 = matches.is_present("444");
    let audio = matches.get_one("audio").copied().unwrap_or(false);
    let inputs = matches.get_one("inputs").copied().unwrap_or(true);

    // Create window
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    let pipeline_hook = Box::new(ClientPipelineHook::new_from_event_loop(
        event_loop.create_proxy(),
    ));

    let mut window = window::Window::new(event_loop, pipeline_hook.get_state_reader()).unwrap();

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
    let connect_config =
        libclient_rs::ConnectConfig::default(streamer_ip.as_str(), STREAMER_HOST, STREAMER_CERT)
            .unwrap();

    if let Some(port) = matches.get_one("port") {
        connect_config.set_streamer_port(*port);
    }

    // Create StreamSessionConfig
    let session_config = libclient_rs::StreamingSessionConfig::default(protocol).unwrap();

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

    let video_config =
        libclient_rs::VideoSessionConfig::default(&window_handle).unwrap();
    video_config.set_codec(video_codec);
    video_config.set_yuv444(yuv444);

    session_config.set_video(video_config);
    session_config.set_audio(audio);

    let input_config = libclient_rs::InputSessionConfig::default(&input_pipeline).unwrap();
    session_config.set_input(input_config);

    // Create client
    let client = Client::new(&connect_config, &session_config).unwrap();
    window.set_client(client.clone()).unwrap();

    // Create Shortcuts handler
    input_pipeline.set_hook(pipeline_hook).unwrap();

    // Setup input routine
    // Window => InputRouter
    if inputs {
        window.set_input_pipeline(input_pipeline).unwrap();
    }

    client.start().unwrap();

    // Process window loop
    window.run_loop().unwrap();
}
