mod video_layout;
#[allow(clippy::module_inception)]
mod window;
mod window_winit;
mod windows_statemachine;

pub use video_layout::{Position, RectSize, VideoLayout};
pub(crate) use window::{Window, WindowControl};
pub(crate) use window_winit::WindowControlWinit;
pub(crate) use windows_statemachine::WindowsStateMachine;
