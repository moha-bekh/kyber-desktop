use super::{Position, Size};

#[derive(Clone, Copy, Debug)]
pub struct VideoLayout {
    pub window_size: Size,
    pub host_size: Size,
    pub video_size: Size,
    pub video_pos: Position,
}

impl VideoLayout {
    pub fn update_display_layout(&mut self) {
        let window_ratio = self.window_size.width / self.window_size.height;
        let stream_ratio = self.host_size.width / self.host_size.height;

        if window_ratio > stream_ratio {
            // Vertical black bars
            self.video_size = Size {
                width: self.window_size.height * stream_ratio,
                height: self.window_size.height,
            };

            self.video_pos = Position {
                x: (self.window_size.width - self.video_size.width) / 2.0,
                y: 0.0,
            };
        } else {
            // Horizontal black bars
            self.video_size = Size {
                width: self.window_size.width,
                height: self.window_size.width / stream_ratio,
            };

            self.video_pos = Position {
                x: 0.0,
                y: (self.window_size.height - self.video_size.height) / 2.0,
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

        let scale = self.host_size.width / self.video_size.width;

        Some(Position {
            x: local_video_pos.x * scale,
            y: local_video_pos.y * scale,
        })
    }

    pub fn host_to_local(&self, host_pos: &Position) -> Position {
        let scale = self.video_size.width / self.host_size.width;

        Position {
            x: self.video_pos.x + host_pos.x * scale,
            y: self.video_pos.y + host_pos.y * scale,
        }
    }
}
