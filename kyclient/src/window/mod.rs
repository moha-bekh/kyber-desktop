// Project Kyber: mod.rs
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

#[allow(clippy::module_inception)]
mod window;
mod window_winit;
mod windows_statemachine;

pub(crate) use kynput_rs::video_layout::{Position, Size, VideoLayout};

pub(crate) use window::{Window, WindowControl};
pub(crate) use window_winit::WindowControlWinit;
pub(crate) use windows_statemachine::WindowsStateMachine;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub position: Position,
    pub size: Size,
}

impl Rect {
    fn is_position_inside(&self, pos: &Position) -> bool {
        pos.x >= self.position.x
            && pos.x < self.size.width + self.position.x
            && pos.y >= self.position.y
            && pos.y < self.size.height + self.position.y
    }
}
