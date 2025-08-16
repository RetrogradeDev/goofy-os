use alloc::{string::String, vec::Vec};

use crate::framebuffer::{Color, FrameBufferWriter};

pub enum Shape {
    Rectangle {
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: Color,
        filled: bool,

        hide: bool,
    },
    Text {
        x: usize,
        y: usize,
        content: String,
        color: Color,
        fill_bg: bool,

        hide: bool,
    },
}

impl Shape {
    pub fn render(&self, framebuffer: &mut FrameBufferWriter, offset_x: usize, offset_y: usize) {
        match self {
            Shape::Rectangle {
                x,
                y,
                width,
                height,
                color,
                filled,
                hide,
            } => {
                if *hide {
                    return;
                }

                if *filled {
                    framebuffer.draw_rect(
                        (*x + offset_x, *y + offset_y),
                        (*x + width - 1 + offset_x, *y + height - 1 + offset_y),
                        *color,
                    );
                } else {
                    framebuffer.draw_rect_outline(
                        (*x + offset_x, *y + offset_y),
                        (*x + width - 1 + offset_x, *y + height - 1 + offset_y),
                        *color,
                    );
                }
            }
            Shape::Text {
                x,
                y,
                content,
                color,
                fill_bg,
                hide,
            } => {
                if *hide {
                    return;
                }

                framebuffer.draw_raw_text(content, *x + offset_x, *y + offset_y, *color, *fill_bg);
            }
        }
    }
}

pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub background_color: Color,
    pub just_fill_bg: bool,
    pub shapes: Vec<Shape>,
    pub is_dirty: bool,
}

impl Surface {
    pub fn new(width: usize, height: usize, background_color: Color) -> Self {
        Self {
            width,
            height,
            background_color,
            just_fill_bg: false,
            shapes: Vec::new(),
            is_dirty: true,
        }
    }

    pub fn add_shape(&mut self, shape: Shape) -> usize {
        self.shapes.push(shape);
        self.is_dirty = true;

        return self.shapes.len() - 1;
    }

    pub fn render(
        &mut self,
        framebuffer: &mut FrameBufferWriter,
        offset_x: usize,
        offset_y: usize,
        force: bool,
    ) -> bool {
        if self.is_dirty || force {
            if self.just_fill_bg {
                framebuffer.fill(self.background_color.r); // Assume `r` is the brightness level
            } else {
                framebuffer.draw_rect(
                    (offset_x, offset_y),
                    (offset_x + self.width - 1, offset_y + self.height - 1),
                    self.background_color,
                ); // TODO: Check if "regions" are dirty instead of full framebuffer, this is extremely slow
            }

            for shape in &self.shapes {
                shape.render(framebuffer, offset_x, offset_y);
            }
            self.is_dirty = false;

            return true;
        }
        false
    }
}
