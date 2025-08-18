use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::{
    desktop::calculator::Calculator,
    framebuffer::{Color, FrameBufferWriter},
    surface::{Rect, Surface},
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
    pub dragging_offset: Option<(i16, i16)>,
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
            dragging_offset: None,
        }
    }

    /// Get the window bounds including titlebar and border
    pub fn get_full_bounds(&self) -> Rect {
        Rect::new(
            self.x.saturating_sub(1),
            self.y.saturating_sub(20),
            self.width + 2,
            self.height + 21,
        )
    }

    /// Get the window content bounds (just the surface area)
    pub fn get_content_bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// Check if this window intersects with the given dirty regions
    pub fn intersects_dirty_regions(&self, dirty_regions: &[Rect]) -> bool {
        let window_bounds = self.get_full_bounds();
        dirty_regions
            .iter()
            .any(|rect| rect.intersects(&window_bounds))
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

    pub fn render_decorations(&self, framebuffer: &mut FrameBufferWriter) {
        // Window outline
        framebuffer.draw_rect_outline(
            (self.x - 1, self.y - 1),
            (self.x + self.width, self.y + self.height),
            Color::BLACK,
        );

        // Titlebar
        framebuffer.draw_rect(
            (self.x - 1, self.y - 20),
            (self.x + self.width, self.y),
            Color::BLACK,
        );
        framebuffer.draw_raw_text(&self.title, self.x + 5, self.y - 15, Color::WHITE, false);

        // Close button
        framebuffer.draw_rect(
            (self.x + self.width - 20, self.y - 20),
            (self.x + self.width, self.y),
            Color::RED,
        );
        framebuffer.draw_line(
            (self.x + self.width - 15, self.y - 15),
            (self.x + self.width - 5, self.y - 5),
            Color::WHITE,
        );
        framebuffer.draw_line(
            (self.x + self.width - 15, self.y - 5),
            (self.x + self.width - 5, self.y - 15),
            Color::WHITE,
        );
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

    pub fn render(
        &mut self,
        framebuffer: &mut FrameBufferWriter,
        desktop_dirty_regions: &[Rect],
    ) -> bool {
        let mut did_render = false;

        for window in &mut self.windows {
            // Only render window if it intersects with dirty regions or window itself is dirty
            let intersects_dirty = window.intersects_dirty_regions(desktop_dirty_regions);
            let should_render = window.surface.is_dirty || intersects_dirty;

            if window.render(framebuffer, should_render) {
                did_render = true;
            }

            if did_render {
                // Always render decorations when we render the window
                window.render_decorations(framebuffer);
            }
        }

        did_render
    }

    /// Handles mouse click events on windows.
    /// Returns: (handled, dirty_region)
    pub fn handle_mouse_click(
        &mut self,
        x: i16,
        y: i16,
    ) -> (bool, Option<(usize, usize, usize, usize)>) {
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
                    return (true, None);
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
                let bounds = (
                    window.x - 1,
                    window.y - 20,
                    window.width + 2,
                    window.height + 21,
                ); // Don't forget the outline and title bar :)

                self.windows.retain(|w| w.id != window_id);
                return (true, Some(bounds));
            }
        }

        (false, None)
    }

    pub fn handle_mouse_down(&mut self, x: i16, y: i16) -> bool {
        for window in &mut self.windows {
            if x as usize >= window.x
                && x as usize <= window.x + window.width - 20
                && y as usize >= window.y - 20
                && y as usize <= window.y
            {
                window.dragging_offset = Some((x, y));
                return true;
            }
        }
        false
    }

    pub fn handle_mouse_move(&mut self, x: i16, y: i16) -> Option<(usize, usize, usize, usize)> {
        for window in &mut self.windows {
            if let Some(offset) = window.dragging_offset {
                let delta_x = x - offset.0;
                let delta_y = y - offset.1;

                window.dragging_offset = Some((x, y));

                let prev_x = window.x;
                let prev_y = window.y;

                window.x = (window.x as i16).saturating_add(delta_x).max(1) as usize;
                window.y = (window.y as i16).saturating_add(delta_y).max(20) as usize;

                let (x, width) = if delta_x < 0 {
                    (
                        window.x.saturating_sub(1),
                        window.width.saturating_add(-delta_x as usize + 2),
                    )
                } else {
                    (
                        prev_x.saturating_sub(1),
                        window.width.saturating_add(delta_x as usize + 2),
                    )
                };

                let (y, height) = if delta_y < 0 {
                    (
                        window.y.saturating_sub(20),
                        window.height.saturating_add(-delta_y as usize + 21),
                    )
                } else {
                    (
                        prev_y.saturating_sub(20),
                        window.height.saturating_add(delta_y as usize + 21),
                    )
                };

                return Some((x, y, width, height));
            }
        }

        None
    }

    pub fn handle_mouse_release(&mut self) {
        for window in &mut self.windows {
            window.dragging_offset = None;
        }
    }
}

pub fn launch_calculator(window_manager: &mut WindowManager) {
    window_manager.add_window(Window::new(
        100,
        100,
        205,
        315,
        1,
        "Calculator".to_string(),
        Some(crate::desktop::window_manager::Application::Calculator(
            Calculator::new(),
        )),
    ));
}
