// Project Kyber: icon.rs
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

use log::warn;
use winit::window::Icon;

use crate::{Error, Result};

#[cfg(target_os = "linux")]
pub(crate) fn load_icon() -> Result<Icon> {
    // Get icon path
    let Ok(mut path) = std::env::current_exe() else {
        return Err(Error::IconLoad);
    };

    // Remove exe + /bin
    path.pop();
    path.pop();
    // Append /share/kyber.png
    path.push("share");
    path.push("kyber.ico");

    // Load icon
    let icon = std::fs::read(&path).map_err(|e| {
        warn!("Couldn't read {path:?}: {e:?}");
        Error::IconLoad
    })?;

    // Convert to winit icon
    let icon =
        image::load_from_memory_with_format(&icon, image::ImageFormat::Ico).map_err(|e| {
            warn!("Couldn't load icon: {e:?}");
            Error::IconLoad
        })?;
    let icon = icon.into_rgba8();

    let (width, height) = icon.dimensions();
    winit::window::Icon::from_rgba(icon.into_raw(), width, height).map_err(|e| {
        warn!("Winit icon creation failed: {e:?}");
        Error::IconLoad
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn load_icon() -> Result<Icon> {
    use winit::platform::windows::IconExtWindows;

    const ICON_ID: u16 = 1;

    winit::window::Icon::from_resource(ICON_ID, None).map_err(|e| {
        warn!("Couldn't read icon from ressource: {e:?}");
        Error::IconLoad
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn load_icon() -> Result<Icon> {
    Err(Error::IconLoad)
}
