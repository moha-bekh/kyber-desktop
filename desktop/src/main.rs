mod client_proxy;
mod cursor_handler;
mod error;
mod input_handler;
mod pipeline_hook;
mod video_layout;
mod window;

use std::sync::Arc;

use log::{info, LevelFilter};
use winit::event_loop::EventLoop;

use libclient_rs::{Client, StreamingProtocol, WindowHandle};
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
        .arg(clap::Arg::with_name("STREAMER_IP").required(true).index(1))
        .get_matches();

    let streamer_ip = String::from(matches.value_of("STREAMER_IP").unwrap());

    // Create window
    let event_loop = EventLoop::<UserEvent>::with_user_event();

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

    // Create client
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

    let config = libclient_rs::Config::default(
        STREAMER_HOST,
        streamer_ip.as_str(),
        STREAMER_CERT,
        StreamingProtocol::Rtp,
        &input_pipeline,
        &window_handle,
    )
    .unwrap();

    config.set_yuv444(matches.is_present("444"));
    config.set_audio(matches.get_one("audio").copied().unwrap_or(false));
    config.set_inputs(matches.get_one("inputs").copied().unwrap_or(true));

    let client = Client::new(&config);

    window.set_client(client.clone()).unwrap();

    // Create Shortcuts handler
    input_pipeline.set_hook(pipeline_hook).unwrap();

    // Setup input routine
    // Window => InputRouter
    window.set_input_pipeline(input_pipeline).unwrap();

    client.start().unwrap();

    // Process window loop
    window.run_loop().unwrap();
}
