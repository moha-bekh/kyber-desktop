// Project Kyber: error.rs
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

use crate::event_loop::UserEvent;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    IO(std::io::Error),
    WindowOs(winit::error::OsError),
    ThreadPanicked,
    IconLoad,
    NoClient,
    UnknownWindow,
    Winit,
    WinitEventLoopClosed,
    Libclient(libclient_rs::Error),
    Input(libkynput_rs::Error),
    // Window errors
    WindowFailed(winit::error::ExternalError),
    // Configuration errors
    Config(String),
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

impl From<winit::event_loop::EventLoopClosed<UserEvent>> for Error {
    fn from(_: winit::event_loop::EventLoopClosed<UserEvent>) -> Error {
        Error::WinitEventLoopClosed
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IO(e) => write!(f, "IO error: {}", e),
            Error::WindowOs(e) => write!(f, "Window OS error: {}", e),
            Error::ThreadPanicked => write!(f, "Thread panicked"),
            Error::IconLoad => write!(f, "Failed to load icon"),
            Error::NoClient => write!(f, "No client"),
            Error::UnknownWindow => write!(f, "Unknown window"),
            Error::Winit => write!(f, "Winit error"),
            Error::WinitEventLoopClosed => write!(f, "Event loop closed"),
            Error::Libclient(e) => write!(f, "Libclient error: {:?}", e),
            Error::Input(e) => write!(f, "Input error: {:?}", e),
            Error::WindowFailed(e) => write!(f, "Window failed: {}", e),
            Error::Config(msg) => write!(f, "{}", msg),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
