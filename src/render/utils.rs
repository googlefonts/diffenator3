use image::{GenericImage, GrayImage, ImageBuffer};
use skrifa::outline::OutlinePen;
use zeno::{Command, PathBuilder};

/// Compute the bounding box of a path, in a terrible but fast way
///
/// This computes the bounding box of all control points in the path. Where
/// curves extend beyond the control points, the real bounding box will be larger
/// than the one returned here. But it's good enough for well-behaved curves
/// with points on extrema.
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

/// Make two images the same size by padding the smaller one with white
///
/// The smaller image is anchored to the top left corner of the larger image.
pub fn make_same_size(image_a: GrayImage, image_b: GrayImage) -> (GrayImage, GrayImage) {
    let max_width = image_a.width().max(image_b.width());
    let max_height = image_a.height().max(image_b.height());
    let mut a = ImageBuffer::new(max_width, max_height);
    let mut b = ImageBuffer::new(max_width, max_height);
    a.copy_from(&image_a, 0, 0).unwrap();
    b.copy_from(&image_b, 0, 0).unwrap();
    (a, b)
}

/// Compare two images and return the count of differing pixels
pub fn count_differences(img_a: GrayImage, img_b: GrayImage, fuzz: u8) -> usize {
    let (img_a, img_b) = make_same_size(img_a, img_b);
    let img_a_vec = img_a.to_vec();
    img_a_vec
        .iter()
        .zip(img_b.to_vec())
        .filter(|(&cha, chb)| cha.abs_diff(*chb) > fuzz)
        .count()
}
