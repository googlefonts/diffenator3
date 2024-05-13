use skrifa::outline::OutlinePen;
use zeno::{Command, PathBuilder};

pub(crate) fn terrible_bounding_box(pen_buffer: &[Command]) -> (f32, f32, f32, f32) {
    let mut max_x: f32 = 0.0;
    let mut min_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    let mut min_y: f32 = 0.0;

    for command in pen_buffer {
        match command {
            Command::MoveTo(to) => {
                min_x = min_x.min(to.x);
                max_x = max_x.max(to.x);
                min_y = min_y.min(to.y);
                max_y = max_y.max(to.y);
            }
            Command::LineTo(to) => {
                min_x = min_x.min(to.x);
                max_x = max_x.max(to.x);
                min_y = min_y.min(to.y);
                max_y = max_y.max(to.y);
            }
            Command::QuadTo(ctrl, to) => {
                min_x = min_x.min(ctrl.x);
                max_x = max_x.max(ctrl.x);
                min_y = min_y.min(ctrl.y);
                max_y = max_y.max(ctrl.y);
                min_x = min_x.min(to.x);
                max_x = max_x.max(to.x);
                min_y = min_y.min(to.y);
                max_y = max_y.max(to.y);
            }
            Command::CurveTo(ctrl0, ctrl1, to) => {
                min_x = min_x.min(ctrl0.x);
                max_x = max_x.max(ctrl0.x);
                min_y = min_y.min(ctrl0.y);
                max_y = max_y.max(ctrl0.y);
                min_x = min_x.min(ctrl1.x);
                max_x = max_x.max(ctrl1.x);
                min_y = min_y.min(ctrl1.y);
                max_y = max_y.max(ctrl1.y);
                min_x = min_x.min(to.x);
                max_x = max_x.max(to.x);
                min_y = min_y.min(to.y);
                max_y = max_y.max(to.y);
            }
            Command::Close => {}
        };
    }
    (min_x, min_y, max_x, max_y)
}

#[derive(Default)]
pub struct RecordingPen {
    pub buffer: Vec<Command>,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl OutlinePen for RecordingPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.buffer.move_to([self.offset_x + x, self.offset_y + y]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.buffer.line_to([self.offset_x + x, self.offset_y + y]);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.buffer.quad_to(
            [self.offset_x + cx0, self.offset_y + cy0],
            [self.offset_x + x, self.offset_y + y],
        );
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.buffer.curve_to(
            [self.offset_x + cx0, self.offset_y + cy0],
            [self.offset_x + cx1, self.offset_y + cy1],
            [self.offset_x + x, self.offset_y + y],
        );
    }

    fn close(&mut self) {
        self.buffer.close();
    }
}
