#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Weak};

use log::warn;
use winit::{
    event::{DeviceEvent, WindowEvent},
    window::WindowId,
};

use libkynput_rs::Cursor;

use super::window::{CursorGrabMode, CursorVisible};
use super::{Position, Size, Window, WindowControl};
use crate::error::{Error, Result};
use crate::inputs::InputContext;

// After processing a new cursor, we may take various actions
enum CursorAction {
    // Wrap the cursor in a window at a specific location
    WrapCursor {
        window_id: WindowId,
        position: Position,
    },
    // Switch to absolute mode
    SwitchAbsolute {
        window_id: WindowId,
    },
    // Switch to relative mode
    SwitchRelative {
        window_id: Option<WindowId>,
    },
}

pub(crate) struct WindowsStateMachine<W>
where
    W: WindowControl,
{
    input_ctx: Option<Weak<InputContext>>,
    windows: Vec<Window<W>>,
    windows_map: HashMap<WindowId, usize>,
    active_cursor: Option<Rc<Cursor>>,
    last_active_window: Option<WindowId>,
    fullscreen: bool,
}

impl<W> WindowsStateMachine<W>
where
    W: WindowControl,
{
    pub(crate) fn new() -> Self {
        Self {
            input_ctx: None,
            windows: vec![],
            windows_map: HashMap::new(),
            active_cursor: None,
            last_active_window: None,
            fullscreen: false,
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

    pub(crate) fn add_window(&mut self, window: Window<W>) {
        let window_id = window.id();
        let index = self.windows.len();
        self.windows.push(window);
        self.windows_map.insert(window_id, index);
    }

    pub(crate) fn iter_windows(&self) -> impl Iterator<Item = &Window<W>> {
        self.windows.iter()
    }

    fn get_window(&self, id: &WindowId) -> Option<&Window<W>> {
        let index = self.windows_map.get(id)?;
        Some(&self.windows[*index])
    }

    fn get_window_mut(&mut self, id: &WindowId) -> Option<&mut Window<W>> {
        let index = self.windows_map.get(id)?;
        Some(&mut self.windows[*index])
    }

    pub(crate) fn focused(&mut self, id: WindowId, focused: bool) -> Result<()> {
        // Update window focus status as reported by the OS
        if let Some(window) = self.get_window_mut(&id) {
            window.set_focused(focused);

            if !focused {
                window.set_last_cursor_pos(None);
            }
        }

        // Check if we have focus. Check that at least one of those conditions
        // is true:
        // - OS reports that at least one window currently has focus
        // - We have detected that the cursor is within one window
        let one_window_focused = self.windows.iter().any(|window| window.focused());

        let one_window_has_cursor = self
            .windows
            .iter()
            .any(|window| window.last_cursor_pos().is_some());

        // Apply focus status
        let focused = one_window_focused || one_window_has_cursor;

        if focused && self.windows.len() == 1 {
            // In single display mode, grab the cursor if focus is gained again.
            let grab_mod = self.cursor_grab_mode();

            if let Some(window) = self
                .windows
                .iter_mut()
                .find(|window| window.focused() && window.is_fullscreen())
            {
                window.grab_cursor(grab_mod);
            }
        } else if !focused {
            if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
                input_ctx.flush_keys()?;
            }

            for window in self.windows.iter_mut() {
                window.grab_cursor(CursorGrabMode::None);
            }
        }

        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.set_global_processing_state(focused)?;
        }

        Ok(())
    }

    fn send_position(
        &self,
        input_ctx: &Arc<InputContext>,
        window_id: WindowId,
        position: Position,
        capture_cursor: bool,
    ) -> Result<()> {
        let Some(window) = self.get_window(&window_id) else {
            return Ok(());
        };

        if !capture_cursor && !window.position_inside_video(&position) {
            return Ok(());
        }

        if let Some(projected_pos) = window.local_to_host(&position, capture_cursor) {
            input_ctx.send_mouse_position(window.display_id(), projected_pos);
        }

        Ok(())
    }

    pub(crate) fn sync_cursor_position(&mut self) {
        struct WindowIdPosition {
            window_id: WindowId,
            position: Position,
        }

        // Find a window that has the cursor
        let position = self.windows.iter().find_map(|window| {
            window
                .last_cursor_pos()
                .filter(|position| window.position_inside_video(position))
                .map(|position| WindowIdPosition {
                    window_id: window.id(),
                    position,
                })
        });

        let position = if let Some(position) = position {
            position
        } else {
            // If cursor was outside the video, assume the cursor was
            // on the center of the first window.
            let Some(window) = self.windows.first() else {
                return;
            };

            let window_size = window.size();
            let position = Position {
                x: window_size.width / 2.0,
                y: window_size.height / 2.0,
            };

            WindowIdPosition {
                window_id: window.id(),
                position,
            }
        };

        if let Some(window) = self.get_window_mut(&position.window_id) {
            window.set_last_cursor_pos(Some(Position {
                x: position.position.x,
                y: position.position.y,
            }));

            window.gain_focus();
        }

        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            let ret = self.send_position(&input_ctx, position.window_id, position.position, false);
            if let Err(err) = ret {
                warn!("Failed to sync host cursor position: {err:?}");
            }
        }
    }

    fn get_window_from_position(
        &mut self,
        id: WindowId,
        position: Position,
    ) -> Result<(WindowId, Position)> {
        // Single window shortcut: assume that the input position comes from the window
        if self.windows.len() == 1 {
            return Ok((id, position));
        }

        let absolute_pos = if let Some(window) = self.get_window(&id) {
            // The position is within the current window, use this window
            if window.position_inside_video(&position) {
                return Ok((id, position));
            }

            // Else, get the positon in Virtual screen coordinates, aka "Absolute position"
            let Some(inner_pos) = window.position() else {
                warn!("Fail to get Window {id:?} inner position");
                return Err(Error::Winit);
            };

            Position {
                x: inner_pos.x + position.x,
                y: inner_pos.y + position.y,
            }
        } else {
            warn!("Unknown Window {id:?}");
            return Err(Error::UnknownWindow);
        };

        // Try to find a window where the position may be located
        let ret = self
            .windows
            .iter()
            .filter(|window| window.id() != id)
            .find_map(|window| {
                // Check if the position is within the window video
                let Some(video_position) = window.video_absolute_position() else {
                    warn!("Fail to get Window {id:?} video position");
                    return None;
                };

                if !video_position.is_position_inside(&absolute_pos) {
                    return None;
                }

                // If true, switch from absolute position to a position relative
                // to the matching window
                let inner_pos = window.position()?;

                let position = Position {
                    x: absolute_pos.x - inner_pos.x,
                    y: absolute_pos.y - inner_pos.y,
                };

                Some((window.id(), position))
            });

        // Otherwise, keep the initial position
        Ok(ret.unwrap_or((id, position)))
    }

    fn window_from_display_id(&mut self, display_id: u32) -> Option<&mut Window<W>> {
        self.windows.iter_mut().find_map(|window| {
            if window.display_id() == display_id {
                Some(window)
            } else {
                None
            }
        })
    }

    pub(crate) fn cursor_moved(&mut self, id: WindowId, position: Position) -> Result<()> {
        if self.mouse_mode_relative() {
            return Ok(());
        }

        let Ok((id, position)) = self.get_window_from_position(id, position) else {
            self.windows.iter_mut().for_each(|window| {
                window.set_last_cursor_pos(None);
            });

            return Ok(());
        };

        let active_cursor = self.active_cursor.clone();

        let (window_id, entered_window) = if let Some(window) = self.get_window_mut(&id) {
            // Avoid to send the same MouseMove multiple time
            if let Some(last_cursor_pos) = window.last_cursor_pos() {
                if last_cursor_pos == position {
                    return Ok(());
                }
            }

            let last_cursor_pos = window.set_last_cursor_pos(Some(position));
            let entered_window = last_cursor_pos.is_none();

            // Assume the Host cursor is still the same after entering the window.
            // We can't be sure that there will be any new cursor sent by the host
            // when changing screen.
            if let Some(cursor) = active_cursor {
                if entered_window && cursor.visible {
                    window.set_cursor_visible(true);
                    window.set_host_cursor_visible(true);
                    window.set_cursor_rgba((*cursor).clone());
                }
            }

            (window.id(), entered_window)
        } else {
            return Ok(());
        };

        if entered_window {
            self.cursor_entered(window_id, true)?;

            // The cursor has entered the window. Maybe one other window has the
            // information of cursor position, erase it.
            self.windows
                .iter_mut()
                .filter(|window| window.id() != window_id)
                .for_each(|window| {
                    window.set_last_cursor_pos(None);
                });
        }

        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            self.send_position(
                &input_ctx,
                window_id,
                position,
                input_ctx.has_one_mouse_button_pressed(),
            )?;
        };

        Ok(())
    }

    pub(crate) fn cursor_entered(&mut self, id: WindowId, entered: bool) -> Result<()> {
        self.last_active_window = Some(id);

        let multi_window = self.windows.len() > 1;

        if let Some(window) = self.get_window_mut(&id) {
            if !window.cursor_entered() {
                window.set_cursor_entered(entered);
            }

            if multi_window && entered && window.is_fullscreen() {
                window.gain_focus();
            } else if !entered {
                window.set_last_cursor_pos(None);
            }
        }

        let entered = self.windows.iter().any(|window| window.cursor_entered());

        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.cursor_inside_window(entered)?;
        }

        Ok(())
    }

    fn cursor_grab_mode(&self) -> CursorGrabMode {
        if self.windows.len() > 1 {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::Confined
        }
    }

    pub(crate) fn resized(&mut self, id: WindowId, size: Size) {
        let cursor_grab_mode = self.cursor_grab_mode();
        let single_window = self.windows.len() == 1;

        if let Some(window) = self.get_window_mut(&id) {
            window.resized(size);

            // The video position has changed, update the grab area
            if single_window {
                if window.is_fullscreen() {
                    window.grab_cursor(cursor_grab_mode);
                } else {
                    window.grab_cursor(CursorGrabMode::None);
                }
            }
        }
    }

    pub(crate) fn device_event(&self, event: DeviceEvent) {
        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.device_event(&event);
        }
    }

    pub(crate) fn window_event(&self, id: WindowId, event: WindowEvent) {
        let scale_factor = {
            let Some(window) = self.get_window(&id) else {
                return;
            };

            window.scale_factor()
        };

        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.window_event(&event, scale_factor);
        }
    }

    pub(crate) fn display_list_updated(&mut self, displays: &[libclient_rs::Display]) {
        for window in self.windows.iter_mut() {
            let Some(display) = displays.iter().find(|&d| d.id == window.display_id()) else {
                continue;
            };

            window.set_host_display_size(display);

            // The video position may have changed, update the grab area
            window.grab_cursor(window.grab_mode());
        }
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;

        for window in self.windows.iter_mut() {
            window.set_fullscreen(self.fullscreen);
        }
    }

    fn mouse_mode_relative(&self) -> bool {
        if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
            input_ctx.mouse_relative()
        } else {
            false
        }
    }

    fn handle_cursor(&mut self, cursor: libkynput_rs::Cursor) -> Option<Vec<CursorAction>> {
        // If Relative mode must be enabled, we need to grab the cursor
        // within the last active window. Otherwise on X11, the cursor will be
        // wrapped into the grabbed window.
        let last_active_window = self.last_active_window;

        // Relative mode should only be enabled if there are no visible cursor
        // on the host side. On Windows Host, it is possible to have more that
        // one visible cursor when all new state haven't been received.
        let host_visible_count = self
            .windows
            .iter()
            .filter(|window| window.host_cursor_visible() == CursorVisible::True)
            .count();

        let relative_mode = self.mouse_mode_relative();

        let window = self.window_from_display_id(cursor.display_id)?;
        let window_id = window.id();

        let mut actions = vec![];

        // MousePosition and MouseMove are always produced on client side.
        // But only one must be sent, depending of the cursor visibility:
        // - Cursor visible: Send absolute position
        // - Cursor not visible: Send relative moves
        if cursor.visible {
            // Cursor must be restored before updating its shape.
            // Otherwise on Linux, the default system cursor will be shown.
            let was_visible = window.set_cursor_visible(true);
            window.set_host_cursor_visible(true);

            if relative_mode && was_visible == CursorVisible::False {
                actions.push(CursorAction::SwitchAbsolute { window_id });
            }

            // If cursor wasn't previously visible, it must be moved at the position
            // indicated by the Host. There is a drift between client cursor position,
            // and host cursor position when injecting relative mouse move.
            //
            // To fix this issue, the Host sends the cursor position if known,
            // otherwise i32::MAX is sent. Then the client must move its own cursor to
            // match the host cursor coordinates.
            //
            // This should be done only when leaving relative mode.
            let cursor_valid = cursor.x != i32::MAX && cursor.y != i32::MAX;
            if was_visible == CursorVisible::False && cursor_valid {
                let position = window.host_to_local(&Position {
                    x: cursor.x as _,
                    y: cursor.y as _,
                });

                actions.push(CursorAction::WrapCursor {
                    window_id,
                    position,
                });
            }

            window.set_cursor_rgba(cursor.clone());
            self.active_cursor = Some(Rc::new(cursor));
        } else {
            let cursor_was_visible = window.set_host_cursor_visible(false);

            #[allow(clippy::if_same_then_else)]
            if self.windows.len() == 1 && cursor_was_visible == CursorVisible::Undefined {
                actions.push(CursorAction::SwitchRelative {
                    window_id: last_active_window,
                });
            } else if host_visible_count == 1 && cursor_was_visible == CursorVisible::True {
                // Switch to relative mode only when the cursor of the current
                // window hides. On Windows, we can get an hidden cursor from other
                // displays.
                actions.push(CursorAction::SwitchRelative {
                    window_id: last_active_window,
                });
            }
        }

        Some(actions)
    }

    pub(crate) fn set_cursor(&mut self, cursor: libkynput_rs::Cursor) -> Result<()> {
        let cursor_grab_mode = self.cursor_grab_mode();

        let actions = self.handle_cursor(cursor);
        let Some(actions) = actions else {
            return Ok(());
        };

        for action in actions {
            match action {
                CursorAction::WrapCursor {
                    window_id,
                    position,
                } => {
                    if let Some(window) = self.get_window_mut(&window_id) {
                        if window.last_cursor_pos() != Some(position) {
                            // Store this position because it will be raised by winit after calling
                            // set_cursor_position. This is just a local update, not a real MouseMove
                            window.set_last_cursor_pos(Some(position));
                            window.set_cursor_position(position)?;
                        }
                    }

                    // Since we may have switch window, erase known position in other windows.
                    self.windows
                        .iter_mut()
                        .filter(|window| window.id() != window_id)
                        .for_each(|window| {
                            window.set_last_cursor_pos(None);
                        });
                }
                CursorAction::SwitchAbsolute { window_id } => {
                    let single_window = self.windows.len() == 1;
                    if single_window {
                        if let Some(window) = self.get_window_mut(&window_id) {
                            let fullscreen = window.is_fullscreen();
                            if fullscreen {
                                window.grab_cursor(cursor_grab_mode);
                            } else if !fullscreen {
                                window.grab_cursor(CursorGrabMode::None);
                            }
                        }
                    } else {
                        // On multi window setup, it is possible to wrap the cursor inside another window.
                        // This requires to ungrab the cursor at first, otherwise the cursor won't wrap
                        // in the new window.
                        self.windows
                            .iter_mut()
                            .filter(|window| window.grab_mode() != CursorGrabMode::None)
                            .for_each(|window| window.grab_cursor(CursorGrabMode::None));
                    }

                    if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
                        input_ctx.set_mouse_relative(false)?;
                    }
                }
                CursorAction::SwitchRelative { window_id } => {
                    self.windows.iter_mut().for_each(|window| {
                        window.set_cursor_visible(false);
                    });

                    let window_id = if let Some(window_id) = window_id {
                        window_id
                    } else {
                        let Some(window) = self.windows.first() else {
                            warn!("Failed to switch to relative mode: no window");
                            continue;
                        };

                        window.id()
                    };

                    let already_grabbed = self
                        .windows
                        .iter()
                        .any(|window| window.grab_mode() != CursorGrabMode::None);
                    if !already_grabbed {
                        if let Some(window) = self.get_window_mut(&window_id) {
                            window.grab_cursor(CursorGrabMode::Locked);
                        }
                    }

                    self.active_cursor = None;

                    if let Some(input_ctx) = Self::input_ctx(&self.input_ctx) {
                        input_ctx.set_mouse_relative(true)?;
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn left_button_released(&self) {
        let Some(cursor) = self.active_cursor.clone() else {
            return;
        };

        let Some(window) = self
            .windows
            .iter()
            .find(|window| window.last_cursor_pos().is_some())
        else {
            return;
        };

        window.set_cursor_rgba((*cursor).clone());
    }
}
