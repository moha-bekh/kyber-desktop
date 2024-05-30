#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    WindowOs(winit::error::OsError),
    Rejected,
    ThreadPanicked,
    NoClient,
    UnknownWindow,
    Winit,
    Libclient(libclient_rs::Error),
    Input(libkynput_rs::Error),
    // Window errors
    WindowFailed(winit::error::ExternalError),
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

pub type Result<T> = std::result::Result<T, Error>;
