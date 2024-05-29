use std::sync::{Arc, Weak};

use log::warn;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, WindowEvent},
    window::CursorRgba,
};

use libkynput_rs::CursorData;

use super::{Position, Window, WindowControl};
use crate::error::{Error, Result};
use crate::inputs::InputContext;

pub(crate) struct WindowsStateMachine<W>
where
    W: WindowControl,
{
    input_ctx: Option<Weak<InputContext>>,
    window: Option<Window<W>>,
    cursor_visible: bool,
}

impl<W> WindowsStateMachine<W>
where
    W: WindowControl,
{
    pub(crate) fn new() -> Self {
        Self {
            input_ctx: None,
            window: None,
            cursor_visible: true,
        }
    }

    pub(crate) fn set_input_ctx(&mut self, input_ctx: Weak<InputContext>) {
        self.input_ctx = Some(input_ctx);
    }

    pub(crate) fn input_ctx(input_ctx: &Option<Weak<InputContext>>) -> Option<Arc<InputContext>> {
        if let Some(input_ctx) = input_ctx {
            input_ctx.upgrade()
        } else {
            None
        }
    }

    pub(crate) fn set_window(&mut self, window: Window<W>) {
        self.window = Some(window);
    }

    pub(crate) fn window(&self) -> &Option<Window<W>> {
        &self.window
    }

    pub(crate) fn focused(&mut self, focused: bool) -> Result<()> {
        let Some(window) = self.window.as_mut() else {
            return Ok(());
        };

        if !focused {
            Self::input_ctx(&self.input_ctx)
                .ok_or(Error::NoPipeline)?
                .flush_keys()?;

            window.grab_cursor(false);
        } else if window.is_fullscreen() {
            window.grab_cursor(true);
        }

        Self::input_ctx(&self.input_ctx)
            .ok_or(Error::NoPipeline)?
            .set_global_processing_state(focused)?;

        Ok(())
    }

    pub(crate) fn send_position(
        &self,
        input_ctx: &Arc<InputContext>,
        position: PhysicalPosition<i32>,
        capture_cursor: bool,
    ) -> Result<()> {
        let Some(window) = &self.window else {
            return Ok(());
        };

        let projected_pos = window.local_to_host(
            Position {
                x: position.x as u32,
                y: position.y as u32,
            },
            capture_cursor,
        );

        let Some(projected_pos) = projected_pos else {
            return Ok(());
        };

        input_ctx.send_mouse_position(window.display_id(), projected_pos);

        Ok(())
    }

    pub(crate) fn sync_cursor_position(&self) {
        let Some(window) = &self.window else {
            return;
        };

        // Wrap host cursor to match the local cursor position.
        // If cursor was outside the video, assume the cursor was
        // on the center of the window.
        let position = window
            .last_cursor_pos()
            .filter(|position| window.position_inside_video(position))
            .unwrap_or_else(|| {
                let window_size = window.size();

                PhysicalPosition {
                    x: window_size.width as i32 / 2,
                    y: window_size.height as i32 / 2,
                }
            });

        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            let ret = self.send_position(&input_ctx, position, false);
            if let Err(err) = ret {
                warn!("Failed to sync host cursor position: {err:?}");
            }
        }
    }

    pub(crate) fn cursor_moved(&mut self, position: PhysicalPosition<f64>) -> Result<()> {
        // Cast to our internal representation
        let position: PhysicalPosition<i32> = PhysicalPosition {
            x: position.x as _,
            y: position.y as _,
        };

        // Avoid to send the same MouseMove multiple time
        if let Some(window) = &mut self.window {
            if let Some(last_cursor_pos) = window.last_cursor_pos() {
                if last_cursor_pos.x == position.x && last_cursor_pos.y == position.y {
                    return Ok(());
                }
            }

            window.set_last_cursor_pos(position);
        }

        // Project and send the position if an handler is defined
        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            self.send_position(
                &input_ctx,
                position,
                input_ctx.has_one_mouse_button_pressed(),
            )?;
        };

        Ok(())
    }

    pub(crate) fn cursor_entered(&mut self, entered: bool) -> Result<()> {
        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.cursor_inside_window(entered)?;
        }

        Ok(())
    }

    pub(crate) fn resized(&mut self, size: PhysicalSize<u32>) {
        if let Some(window) = &mut self.window {
            window.resized(size);
            window.grab_cursor(window.is_fullscreen());
        }
    }

    pub(crate) fn device_event(&self, event: DeviceEvent) {
        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.device_event(&event);
        }
    }

    pub(crate) fn window_event(&self, event: WindowEvent) {
        let scale_factor = {
            let Some(window) = &self.window else {
                return;
            };

            window.scale_factor()
        };

        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.window_event(&event, scale_factor);
        }
    }

    pub(crate) fn display_list_updated(&mut self, displays: &[libclient_rs::Display]) {
        let Some(window) = &mut self.window else {
            return;
        };

        let Some(display) = displays.iter().find(|&d| d.id == window.display_id()) else {
            warn!("Used display not found in {displays:?}");
            return;
        };

        window.set_host_display_size(display);
        window.grab_cursor(window.is_fullscreen());
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        if let Some(window) = &mut self.window {
            window.toggle_fullscreen();
        }
    }

    pub(crate) fn set_cursor(&mut self, cursor: libkynput_rs::Cursor) -> Result<()> {
        let Some(window) = &mut self.window else {
            return Ok(());
        };

        // MousePosition and MouseMove are always produced on client side.
        // But only one must be sent, depending of the cursor visibility:
        // - Cursor visible: Send absolute position
        // - Cursor not visible: Send relative moves
        if cursor.visible {
            // Cursor must be restored before updating its shape.
            // Otherwise on Linux, the default system cursor will be shown.
            window.set_cursor_visible(true);

            // If cursor wasn't previously visible, it must be moved at the position
            // indicated by the Host. There is a drift between client cursor position,
            // and host cursor position when injecting relative mouse move.
            //
            // To fix this issue, the Host sends the cursor position if known,
            // otherwise i32::MAX is sent. Then the client must move its own cursor to
            // match the host cursor coordinates.
            if !self.cursor_visible && cursor.x != i32::MAX && cursor.y != i32::MAX {
                let position = window.host_to_local(Position {
                    x: cursor.x as u32,
                    y: cursor.y as u32,
                });

                // Store this position because it will be raised by winit after calling
                // set_cursor_position. This is just a local update, not a real MouseMove
                let physical_position = PhysicalPosition::new(position.x as _, position.y as _);
                window.set_last_cursor_pos(physical_position);
                window.set_cursor_position(physical_position)?;
            }

            match cursor.data {
                CursorData::Raw(data) => {
                    window.set_cursor_rgba(CursorRgba {
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

            if !window.is_fullscreen() {
                window.grab_cursor(false);
            }

            if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
                input_ctx.set_mouse_relative(false)?;
            }
        } else {
            window.set_cursor_visible(false);
            window.grab_cursor(true);

            if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
                input_ctx.set_mouse_relative(true)?;
            }
        }

        self.cursor_visible = cursor.visible;

        Ok(())
    }
}
