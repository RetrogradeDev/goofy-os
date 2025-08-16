use alloc::{string::String, vec::Vec};

use crate::{
    framebuffer::{Color, FrameBufferWriter},
    surface::{Shape, Surface},
};

const TITLEBAR_HEIGHT: usize = 20;

pub struct Window {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub id: usize,
    pub title: String,
    pub surface: Surface,
}

impl Window {
    pub fn new(x: usize, y: usize, width: usize, height: usize, id: usize, title: String) -> Self {
        let mut surface = Surface::new(width, height, Color::BLACK);
        surface.add_shape(Shape::Rectangle {
            x: 0,
            y: 0,
            width,
            height: TITLEBAR_HEIGHT,
            color: Color::BLACK,
            filled: true,
            hide: false,
        });
        surface.add_shape(Shape::Text {
            x: 5,
            y: 5,
            content: title.clone(),
            color: Color::WHITE,
            fill_bg: false,
            hide: false,
        });

        Self {
            x,
            y,
            width,
            height,
            id,
            title,
            surface,
        }
    }

    pub fn render(&mut self, framebuffer: &mut FrameBufferWriter, force: bool) -> bool {
        return self.surface.render(framebuffer, self.x, self.y, force);
    }
}

pub struct WindowManager {
    pub windows: Vec<Window>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
        }
    }

    pub fn add_window(&mut self, window: Window) {
        self.windows.push(window);
    }

    pub fn render(&mut self, framebuffer: &mut FrameBufferWriter, force: bool) -> bool {
        let mut did_render = false;
        for window in &mut self.windows {
            if window.render(framebuffer, force) {
                did_render = true;
            }
        }

        return did_render;
    }
}
