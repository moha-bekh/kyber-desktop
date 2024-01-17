#[derive(Clone, Copy, Debug)]
pub struct RectSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoLayout {
    pub window_size: RectSize,
    pub host_size: RectSize,
    pub video_size: RectSize,
    pub video_pos: Position,
}

impl VideoLayout {
    pub fn update_display_layout(&mut self) {
        let window_ratio = self.window_size.width as f64 / self.window_size.height as f64;
        let stream_ratio = self.host_size.width as f64 / self.host_size.height as f64;

        if window_ratio > stream_ratio {
            // Vertical black bars
            self.video_size = RectSize {
                width: (self.window_size.height as f64 * stream_ratio) as u32,
                height: self.window_size.height,
            };

            self.video_pos = Position {
                x: (self.window_size.width - self.video_size.width) / 2,
                y: 0,
            };
        } else {
            // Horizontal black bars
            self.video_size = RectSize {
                width: self.window_size.width,
                height: (self.window_size.width as f64 / stream_ratio) as u32,
            };

            self.video_pos = Position {
                x: 0,
                y: (self.window_size.height - self.video_size.height) / 2,
            };
        }
    }

    pub fn local_to_host(
        &self,
        mut cursor_pos: Position,
        capture_cursor: bool,
    ) -> Option<Position> {
        // Filter cursor that is not above the displayed video
        if capture_cursor {
            let limit_segment = |position, start, end| {
                if position < start {
                    start
                } else if position >= end {
                    end
                } else {
                    position
                }
            };

            cursor_pos.x = limit_segment(
                cursor_pos.x,
                self.video_pos.x,
                self.video_pos.x + self.video_size.width,
            );

            cursor_pos.y = limit_segment(
                cursor_pos.y,
                self.video_pos.y,
                self.video_pos.y + self.video_size.height,
            );
        } else if cursor_pos.x < self.video_pos.x
            || cursor_pos.x >= self.video_size.width + self.video_pos.x
            || cursor_pos.y < self.video_pos.y
            || cursor_pos.y >= self.video_size.height + self.video_pos.y
        {
            return None;
        }

        // Do projection:
        // - Get the position of the cursor within the local video locally displayed
        // - Compute the scale between original Host size and Video size
        // - Convert the cursor position from Video Frame to Host Frame
        let local_video_pos = Position {
            x: cursor_pos.x - self.video_pos.x,
            y: cursor_pos.y - self.video_pos.y,
        };

        let scale = self.host_size.width as f64 / self.video_size.width as f64;

        Some(Position {
            x: (local_video_pos.x as f64 * scale) as u32,
            y: (local_video_pos.y as f64 * scale) as u32,
        })
    }

    pub fn host_to_local(&self, host_pos: Position) -> Position {
        let scale = self.video_size.width as f64 / self.host_size.width as f64;

        Position {
            x: self.video_pos.x + (host_pos.x as f64 * scale) as u32,
            y: self.video_pos.y + (host_pos.y as f64 * scale) as u32,
        }
    }
}
