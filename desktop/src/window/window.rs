use log::{debug, warn};
use raw_window_handle::RawWindowHandle;
use winit::window::WindowId;

use libkynput_rs::Cursor;

use super::{Position, Rect, Size, VideoLayout};
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CursorGrabMode {
    None,
    Locked,
    Confined,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CursorVisible {
    Undefined,
    False,
    True,
}

pub(crate) trait WindowControl {
    fn id(&self) -> WindowId;

    fn set_cursor_visible(&self, visible: bool);

    fn set_cursor_position(&self, position: Position) -> Result<()>;

    fn set_cursor_rgba(&self, cursor: Cursor);

    fn monitor_size(&self) -> Option<Size>;

    fn inner_position(&self) -> Option<Position>;

    fn inner_size(&self) -> Size;

    fn is_fullscreen(&self) -> bool;

    fn set_fullscreen(&self, fullscreen: bool);

    fn raw_window_handle(&self) -> RawWindowHandle;

    fn scale_factor(&self) -> f64;

    fn set_cursor_rect(&self, rect: Option<(Position, Size)>) -> Result<()>;

    fn set_cursor_grab(&self, grab: bool) -> Result<()>;

    fn gain_focus(&self);
}

pub(crate) struct Window<T>
where
    T: WindowControl,
{
    window: T,
    window_size: Size,
    video_layout: VideoLayout,
    display_id: u32,
    cursor_entered: bool,
    last_cursor_pos: Option<Position>,
    focused: bool,
    cursor_visible: CursorVisible,
    host_cursor_visible: CursorVisible,
    grab_mode: CursorGrabMode,
}

impl<T> Window<T>
where
    T: WindowControl,
{
    pub(crate) fn new(window: T, display: &libclient_rs::Display) -> Result<Self> {
        let window_size = window.inner_size();

        let window_size = Size {
            width: window_size.width,
            height: window_size.height,
        };

        let host_size = Size {
            width: display.width as _,
            height: display.height as _,
        };

        let video_layout = VideoLayout::new(window_size, host_size);

        Ok(Self {
            window,
            window_size,
            video_layout,
            display_id: display.id,
            cursor_entered: true,
            last_cursor_pos: None,
            focused: false,
            cursor_visible: CursorVisible::Undefined,
            host_cursor_visible: CursorVisible::Undefined,
            grab_mode: CursorGrabMode::None,
        })
    }

    pub(crate) fn id(&self) -> WindowId {
        self.window.id()
    }

    pub(crate) fn grab_cursor(&mut self, mode: CursorGrabMode) {
        match mode {
            CursorGrabMode::None => {
                let ret = self.window.set_cursor_rect(None);
                if let Err(err) = ret {
                    warn!("Fail during set_cursor_rect(None): {err:?}");
                }

                let ret = self.window.set_cursor_grab(false);
                if let Err(err) = ret {
                    warn!("Fail during set_cursor_grab(false): {err:?}");
                }
            }
            CursorGrabMode::Locked => {
                let ret = self.window.set_cursor_grab(true);
                if let Err(err) = ret {
                    warn!("Fail during set_cursor_grab(true): {err:?}");
                }
            }
            CursorGrabMode::Confined => {
                // The cursor should be contained in the Rect where the video is displayed.
                // However, not all platforms support this feature, and some are only able
                // to contain within the whole window.
                // Try to contain in a Rect at first, and fallback in the whole window contain
                // if not supported.
                let (pos, size) = self.video_layout.video();
                let ret = self.window.set_cursor_rect(Some((pos, size)));

                if ret.is_err() {
                    let ret = self.window.set_cursor_grab(true);
                    if let Err(err) = ret {
                        warn!("Fail during grab_cursor(true): {err:?}");
                    }
                }
            }
        }

        self.grab_mode = mode;
    }

    pub(crate) fn grab_mode(&self) -> CursorGrabMode {
        self.grab_mode
    }

    pub(crate) fn set_host_cursor_visible(&mut self, visible: bool) -> CursorVisible {
        let was_visible = self.host_cursor_visible;

        self.host_cursor_visible = if visible {
            CursorVisible::True
        } else {
            CursorVisible::False
        };

        was_visible
    }

    pub(crate) fn host_cursor_visible(&self) -> CursorVisible {
        self.host_cursor_visible
    }

    pub(crate) fn set_cursor_visible(&mut self, visible: bool) -> CursorVisible {
        let was_visible = self.cursor_visible;

        self.window.set_cursor_visible(visible);

        self.cursor_visible = if visible {
            CursorVisible::True
        } else {
            CursorVisible::False
        };

        was_visible
    }

    pub(crate) fn set_cursor_position(&self, position: Position) -> Result<()> {
        self.window.set_cursor_position(position)?;

        Ok(())
    }

    pub(crate) fn set_cursor_rgba(&self, cursor: Cursor) {
        self.window.set_cursor_rgba(cursor);
    }

    pub(crate) fn resized(&mut self, size: Size) {
        self.window_size.width = size.width;
        self.window_size.height = size.height;

        debug!("Window resized");
        self.video_layout.set_window_size(&self.window_size);
    }

    pub(crate) fn position(&self) -> Option<Position> {
        self.window.inner_position()
    }

    pub(crate) fn size(&self) -> Size {
        Size {
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

        self.video_layout.set_host_size(&Size {
            width: display.width as _,
            height: display.height as _,
        });
    }

    pub(crate) fn is_fullscreen(&self) -> bool {
        self.window.is_fullscreen()
    }

    pub(crate) fn set_fullscreen(&mut self, fullscreen: bool) {
        let is_fullscreen = self.is_fullscreen();

        if is_fullscreen && !fullscreen {
            debug!("Exit fullscreen");
            self.window.set_fullscreen(false);

            self.video_layout.set_window_size(&self.window_size);
        } else if !is_fullscreen && fullscreen {
            debug!("Enter fullscreen");

            if let Some(monitor_size) = self.window.monitor_size() {
                self.video_layout.set_window_size(&monitor_size);
            }

            self.window.set_fullscreen(true);
        }
    }

    pub(crate) fn video_absolute_position(&self) -> Option<Rect> {
        let Some(inner_pos) = self.position() else {
            warn!("Fail to get Window {:?} inner position", self.id());
            return None;
        };

        let (video_pos, video_size) = self.video_layout.video();

        let video_absolute_pos = Position {
            x: inner_pos.x + video_pos.x,
            y: inner_pos.y + video_pos.y,
        };

        Some(Rect {
            position: video_absolute_pos,
            size: video_size,
        })
    }

    pub(crate) fn position_inside_video(&self, position: &Position) -> bool {
        let (video_pos, video_size) = self.video_layout.video();

        let video_rect = Rect {
            position: video_pos,
            size: video_size,
        };

        video_rect.is_position_inside(position)
    }

    pub(crate) fn local_to_host(
        &self,
        cursor_pos: &Position,
        capture_cursor: bool,
    ) -> Option<Position> {
        self.video_layout.local_to_host(*cursor_pos, capture_cursor)
    }

    pub(crate) fn host_to_local(&self, host_pos: &Position) -> Position {
        self.video_layout.host_to_local(host_pos)
    }

    pub(crate) fn display_id(&self) -> u32 {
        self.display_id
    }

    pub(crate) fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }

    pub(crate) fn cursor_entered(&self) -> bool {
        self.cursor_entered
    }

    pub(crate) fn set_cursor_entered(&mut self, entered: bool) {
        self.cursor_entered = entered;
    }

    pub(crate) fn last_cursor_pos(&self) -> Option<Position> {
        self.last_cursor_pos
    }

    pub(crate) fn set_last_cursor_pos(&mut self, pos: Option<Position>) -> Option<Position> {
        let last_cursor_pos = self.last_cursor_pos;
        self.last_cursor_pos = pos;

        last_cursor_pos
    }

    pub(crate) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(crate) fn focused(&self) -> bool {
        self.focused
    }

    pub(crate) fn gain_focus(&self) {
        self.window.gain_focus();
    }
}
