// Project Kyber: window_winit.rs
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

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winit::{dpi, window::Fullscreen};

use kynput_rs::{Cursor, CursorData};
use log::warn;

use super::{Position, Size};
use crate::error::Result;
use crate::window::WindowControl;

pub(crate) struct WindowControlWinit {
    window: winit::window::Window,
}

impl WindowControlWinit {
    pub(crate) fn new(window: winit::window::Window) -> Self {
        Self { window }
    }
}

impl WindowControl for WindowControlWinit {
    fn id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible);
    }

    fn set_cursor_position(&self, position: Position) -> Result<()> {
        let position = dpi::OsPosition::new(position.x, position.y);
        self.window.set_cursor_os_position(position)?;

        Ok(())
    }

    fn set_cursor_rgba(&self, cursor: Cursor) {
        match cursor.data {
            CursorData::Raw(data) => {
                self.window.set_cursor_rgba(winit::window::CursorRgba {
                    xhot: cursor.xhot,
                    yhot: cursor.yhot,
                    width: cursor.width,
                    height: cursor.height,
                    data,
                });
            }
            CursorData::Png(_) => {
                warn!("Got a PNG cursor");
            }
            CursorData::None => {}
        }
    }

    fn monitor_size(&self) -> Option<Size> {
        let monitor = self.window.current_monitor()?;

        let size = monitor.os_size();
        Some(Size {
            width: size.width as _,
            height: size.height as _,
        })
    }

    fn inner_position(&self) -> Option<Position> {
        let Ok(position) = self.window.inner_os_position() else {
            return None;
        };

        Some(Position {
            x: position.x,
            y: position.y,
        })
    }

    fn inner_size(&self) -> Size {
        let size = self.window.inner_os_size();
        Size {
            width: size.width as _,
            height: size.height as _,
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
    }

    fn set_fullscreen(&self, fullscreen: bool) {
        if fullscreen {
            self.window
                .set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            self.window.set_fullscreen(None);
        }
    }

    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }

    fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    fn set_cursor_rect(&self, rect: Option<(Position, Size)>) -> Result<()> {
        let rect = rect.map(|(pos, size)| {
            let pos = dpi::OsPosition::new(pos.x, pos.y);
            let size = dpi::OsSize::new(size.width, size.height);

            (pos, size)
        });

        self.window.set_cursor_os_rect(rect)?;

        Ok(())
    }

    fn set_cursor_grab(&self, grab: bool) -> Result<()> {
        let mode = if grab {
            if cfg!(target_os = "macos") {
                winit::window::CursorGrabMode::Locked
            } else {
                winit::window::CursorGrabMode::Confined
            }
        } else {
            winit::window::CursorGrabMode::None
        };

        self.window.set_cursor_grab(mode)?;

        Ok(())
    }

    fn gain_focus(&self) {
        self.window.focus_window();
    }

    fn winit_window(&self) -> &winit::window::Window {
        &self.window
    }
}
