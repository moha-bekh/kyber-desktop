mod video_layout;
#[allow(clippy::module_inception)]
mod window;
mod window_winit;
mod windows_statemachine;

pub use video_layout::VideoLayout;
pub(crate) use window::{Window, WindowControl};
pub(crate) use window_winit::WindowControlWinit;
pub(crate) use windows_statemachine::WindowsStateMachine;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

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
