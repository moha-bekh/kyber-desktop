#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    WindowOs(winit::error::OsError),
    ChannelSend,
    Rejected,
    ThreadPanicked,
    NoPipeline,
    NoClient,
    Libclient(libclient_rs::Error),
    Input(libkynput_rs::Error),
    TaskJoin(tokio::task::JoinError),
    // Window errors
    WindowFailed(winit::error::ExternalError),
    // Host service errors
    HttpClient(reqwest::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<winit::error::OsError> for Error {
    fn from(err: winit::error::OsError) -> Error {
        Error::WindowOs(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::HttpClient(err)
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for Error {
    fn from(_err: tokio::sync::mpsc::error::SendError<T>) -> Error {
        Error::ChannelSend
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(_: std::sync::PoisonError<T>) -> Error {
        Error::ThreadPanicked
    }
}

impl From<libclient_rs::Error> for Error {
    fn from(err: libclient_rs::Error) -> Error {
        Error::Libclient(err)
    }
}

impl From<libkynput_rs::Error> for Error {
    fn from(err: libkynput_rs::Error) -> Error {
        Error::Input(err)
    }
}

impl From<winit::error::ExternalError> for Error {
    fn from(err: winit::error::ExternalError) -> Error {
        Error::WindowFailed(err)
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Error {
        Error::TaskJoin(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
