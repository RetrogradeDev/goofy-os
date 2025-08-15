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
    },
    Text {
        x: usize,
        y: usize,
        content: String,
        color: Color,
        fill_bg: bool,
    },
}

impl Shape {
    pub fn render(&self, framebuffer: &mut FrameBufferWriter) {
        match self {
            Shape::Rectangle {
                x,
                y,
                width,
                height,
                color,
                filled,
            } => {
                if *filled {
                    framebuffer.draw_rect((*x, *y), (*x + width - 1, *y + height - 1), *color);
                } else {
                    framebuffer.draw_rect_outline(
                        (*x, *y),
                        (*x + width - 1, *y + height - 1),
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
            } => {
                framebuffer.draw_raw_text(content, *x, *y, *color, *fill_bg);
            }
        }
    }
}

pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub shapes: Vec<Shape>,
    pub is_dirty: bool,
}

impl Surface {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            shapes: Vec::new(),
            is_dirty: true,
        }
    }

    pub fn add_shape(&mut self, shape: Shape) -> usize {
        self.shapes.push(shape);
        self.is_dirty = true;

        return self.shapes.len() - 1;
    }

    pub fn render(&mut self, framebuffer: &mut FrameBufferWriter) {
        if self.is_dirty {
            for shape in &self.shapes {
                shape.render(framebuffer);
            }
            self.is_dirty = false;
        }
    }
}
