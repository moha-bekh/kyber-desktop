#[allow(clippy::module_inception)]
mod window;
mod window_winit;
mod windows_statemachine;

pub(crate) use libkynput_rs::video_layout::{Position, Size, VideoLayout};

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
