use alloc::{string::String, vec::Vec};

use crate::{
    desktop::calculator::Calculator,
    framebuffer::{Color, FrameBufferWriter},
    surface::Surface,
};

pub enum Application {
    Calculator(Calculator),
}

pub struct Window {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub id: usize,
    pub title: String,
    pub surface: Surface,
    pub application: Option<Application>,
}

impl Window {
    pub fn new(
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        id: usize,
        title: String,
        application: Option<Application>,
    ) -> Self {
        let background_color = application.as_ref().map_or(Color::BLACK, |app| match app {
            Application::Calculator(_) => Color::GRAY,
        });
        let surface = Surface::new(width, height, background_color);

        Self {
            x,
            y,
            width,
            height,
            id,
            title,
            surface,
            application,
        }
    }

    pub fn render(&mut self, framebuffer: &mut FrameBufferWriter, force: bool) -> bool {
        match &mut self.application {
            Some(Application::Calculator(calculator)) => {
                calculator.render(&mut self.surface);
            }
            _ => (),
        }

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

    pub fn add_window(&mut self, mut window: Window) {
        match &mut window.application {
            Some(Application::Calculator(calculator)) => {
                calculator.init(&mut window.surface);
            }
            _ => (),
        }

        self.windows.push(window);
    }

    pub fn render(&mut self, framebuffer: &mut FrameBufferWriter, force: bool) -> bool {
        let mut did_render = false;
        for window in &mut self.windows {
            if window.render(framebuffer, force) {
                did_render = true;
            }
        }

        if force {
            for window in &mut self.windows {
                // Random outline
                framebuffer.draw_rect_outline(
                    (window.x - 1, window.y - 1),
                    (window.x + window.width, window.y + window.height),
                    Color::BLACK,
                );

                // Titlebar
                framebuffer.draw_rect(
                    (window.x - 1, window.y - 20),
                    (window.x + window.width, window.y),
                    Color::BLACK,
                );
                framebuffer.draw_raw_text(
                    &window.title,
                    window.x + 5,
                    window.y - 15,
                    Color::WHITE,
                    false,
                );

                // Close button
                framebuffer.draw_rect(
                    (window.x + window.width - 20, window.y - 20),
                    (window.x + window.width, window.y),
                    Color::RED,
                );
                framebuffer.draw_line(
                    (window.x + window.width - 15, window.y - 15),
                    (window.x + window.width - 5, window.y - 5),
                    Color::WHITE,
                );
                framebuffer.draw_line(
                    (window.x + window.width - 15, window.y - 5),
                    (window.x + window.width - 5, window.y - 15),
                    Color::WHITE,
                );
            }
        }

        return did_render;
    }

    /// Handles mouse click events on windows.
    /// Returns: (handled, force_redraw)
    pub fn handle_mouse_click(&mut self, x: i16, y: i16) -> (bool, bool) {
        for window in &mut self.windows {
            if x as usize >= window.x
                && x as usize <= window.x + window.width
                && y as usize >= window.y
                && y as usize <= window.y + window.height
            {
                if let Some(Application::Calculator(calculator)) = &mut window.application {
                    let x = (x as usize).saturating_sub(window.x);
                    let y = (y as usize).saturating_sub(window.y);

                    calculator.handle_mouse_click(x, y);
                    return (true, false);
                }
            }
        }

        // Check if the click was on the close button
        for window in &self.windows {
            if x as usize >= window.x + window.width - 20
                && x as usize <= window.x + window.width
                && y as usize >= window.y - 20
                && y as usize <= window.y
            {
                let window_id = window.id; // Rust borrowing checker goes brrr

                self.windows.retain(|w| w.id != window_id);
                return (true, true);
            }
        }

        (false, false)
    }
}
