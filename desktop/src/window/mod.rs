mod video_layout;
#[allow(clippy::module_inception)]
mod window;
mod window_winit;
mod windows_statemachine;

pub use video_layout::VideoLayout;
pub(crate) use window::{Window, WindowControl};
pub(crate) use window_winit::WindowControlWinit;
pub(crate) use windows_statemachine::WindowsStateMachine;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}
