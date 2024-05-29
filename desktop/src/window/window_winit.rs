use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    window::{self, CursorGrabMode, CursorRgba},
};

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

    fn set_cursor_position(&self, position: PhysicalPosition<i32>) -> Result<()> {
        self.window.set_cursor_position(position)?;

        Ok(())
    }

    fn set_cursor_rgba(&self, cursor: CursorRgba) {
        self.window.set_cursor_rgba(cursor);
    }

    fn monitor_size(&self) -> Option<PhysicalSize<u32>> {
        let Some(monitor) = self.window.current_monitor() else {
            return None;
        };

        Some(monitor.size())
    }

    fn inner_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    fn is_fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
    }

    fn set_fullscreen(&self, fullscreen: bool) {
        if fullscreen {
            self.window
                .set_fullscreen(Some(window::Fullscreen::Borderless(None)));
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

    fn set_cursor_rect(
        &self,
        rect: Option<(winit::dpi::Position, winit::dpi::Size)>,
    ) -> Result<()> {
        self.window.set_cursor_rect(rect)?;

        Ok(())
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<()> {
        self.window.set_cursor_grab(mode)?;

        Ok(())
    }
}
