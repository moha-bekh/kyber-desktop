use log::{debug, warn};
use raw_window_handle::RawWindowHandle;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    window::{CursorGrabMode, CursorRgba, WindowId},
};

use super::{Position, RectSize, VideoLayout};
use crate::error::Result;

pub(crate) trait WindowControl {
    fn id(&self) -> WindowId;

    fn set_cursor_visible(&self, visible: bool);

    fn set_cursor_position(&self, position: PhysicalPosition<i32>) -> Result<()>;

    fn set_cursor_rgba(&self, cursor: CursorRgba);

    fn monitor_size(&self) -> Option<PhysicalSize<u32>>;

    fn inner_size(&self) -> PhysicalSize<u32>;

    fn is_fullscreen(&self) -> bool;

    fn set_fullscreen(&self, fullscreen: bool);

    fn raw_window_handle(&self) -> RawWindowHandle;

    fn scale_factor(&self) -> f64;

    fn set_cursor_rect(&self, rect: Option<(winit::dpi::Position, winit::dpi::Size)>)
        -> Result<()>;

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<()>;
}

pub(crate) struct Window<T>
where
    T: WindowControl,
{
    window: T,
    window_size: RectSize,
    video_layout: VideoLayout,
    display_id: u32,
    last_cursor_pos: Option<PhysicalPosition<i32>>,
}

impl<T> Window<T>
where
    T: WindowControl,
{
    pub(crate) fn new(window: T, display: &libclient_rs::Display) -> Result<Self> {
        let window_size = window.inner_size();

        let mut video_layout = VideoLayout {
            window_size: RectSize {
                width: window_size.width,
                height: window_size.height,
            },
            host_size: RectSize {
                width: display.width as _,
                height: display.height as _,
            },
            video_size: RectSize {
                width: 0,
                height: 0,
            },
            video_pos: Position { x: 0, y: 0 },
        };

        video_layout.update_display_layout();

        Ok(Self {
            window,
            window_size: RectSize {
                width: window_size.width,
                height: window_size.height,
            },
            video_layout,
            display_id: display.id,
            last_cursor_pos: None,
        })
    }

    pub(crate) fn grab_cursor(&self, grab: bool) {
        // The cursor should be contained in the Rect where the video is displayed.
        // However, not all platforms support this feature, and some are only able
        // to contain within the whole window.
        // Try to contain in a Rect at first, and fallback in the whole window contain
        // if not supported.
        let ret = if grab {
            use winit::dpi::{Position, Size};

            let pos = Position::Physical(PhysicalPosition::new(
                self.video_layout.video_pos.x as i32,
                self.video_layout.video_pos.y as i32,
            ));

            let size = Size::Physical(PhysicalSize::new(
                self.video_layout.video_size.width,
                self.video_layout.video_size.height,
            ));

            self.window.set_cursor_rect(Some((pos, size)))
        } else {
            self.window.set_cursor_rect(None)
        };

        if ret.is_err() {
            let grab = if grab {
                CursorGrabMode::Confined
            } else {
                CursorGrabMode::None
            };

            let ret = self.window.set_cursor_grab(grab);
            if let Err(err) = ret {
                warn!("Fail during grab_cursor({:?}): {:?}", grab, err);
            }
        }
    }

    pub(crate) fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible);
    }

    pub(crate) fn set_cursor_position(&self, position: PhysicalPosition<i32>) -> Result<()> {
        self.window.set_cursor_position(position)?;

        Ok(())
    }

    pub(crate) fn set_cursor_rgba(&self, cursor: CursorRgba) {
        self.window.set_cursor_rgba(cursor);
    }

    pub(crate) fn resized(&mut self, size: PhysicalSize<u32>) {
        self.window_size.width = size.width;
        self.window_size.height = size.height;

        debug!("Window resized");
        self.video_layout.window_size = RectSize {
            width: size.width,
            height: size.height,
        };

        self.video_layout.update_display_layout();
    }

    pub(crate) fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.window_size.width,
            height: self.window_size.height,
        }
    }

    pub(crate) fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    pub(crate) fn set_host_display_size(&mut self, display: &libclient_rs::Display) {
        if display.id != self.display_id {
            warn!("Trying to update host size from another display");
            return;
        }

        self.video_layout.host_size = RectSize {
            width: display.width as u32,
            height: display.height as u32,
        };

        self.video_layout.update_display_layout();
    }

    pub(crate) fn is_fullscreen(&self) -> bool {
        self.window.is_fullscreen()
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        if self.is_fullscreen() {
            debug!("Exit fullscreen");
            self.window.set_fullscreen(false);

            self.video_layout.window_size = self.window_size;
            self.video_layout.update_display_layout();
        } else {
            debug!("Enter fullscreen");

            if let Some(monitor_size) = self.window.monitor_size() {
                self.video_layout.window_size = RectSize {
                    width: monitor_size.width,
                    height: monitor_size.height,
                };

                self.video_layout.update_display_layout()
            }

            self.window.set_fullscreen(true);
        }
    }

    pub(crate) fn position_inside_video(&self, position: &PhysicalPosition<i32>) -> bool {
        let video_size = &self.video_layout.video_size;
        let video_pos = &self.video_layout.video_pos;

        position.x >= video_pos.x as i32
            && position.x < video_size.width as i32
            && video_pos.y as i32 >= 0
            && position.y < video_size.height as i32
    }

    pub(crate) fn local_to_host(
        &self,
        cursor_pos: Position,
        capture_cursor: bool,
    ) -> Option<Position> {
        self.video_layout.local_to_host(cursor_pos, capture_cursor)
    }

    pub(crate) fn host_to_local(&self, host_pos: Position) -> Position {
        self.video_layout.host_to_local(host_pos)
    }

    pub(crate) fn display_id(&self) -> u32 {
        self.display_id
    }

    pub(crate) fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }

    pub(crate) fn last_cursor_pos(&self) -> Option<PhysicalPosition<i32>> {
        self.last_cursor_pos
    }

    pub(crate) fn set_last_cursor_pos(&mut self, pos: PhysicalPosition<i32>) {
        self.last_cursor_pos = Some(pos);
    }
}
