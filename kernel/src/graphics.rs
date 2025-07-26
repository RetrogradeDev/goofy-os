use crate::{
    framebuffer::{Color, FRAMEBUFFER},
    serial_println,
};

pub fn set_pixel(x: usize, y: usize, color: Color) {
    FRAMEBUFFER
        .get()
        .map(|fb| fb.lock().write_pixel(x, y, color));
}

pub fn draw_line(start: (usize, usize), end: (usize, usize), color: Color) {
    FRAMEBUFFER.get().map(|fb| {
        let mut fb = fb.lock();
        let (x0, y0) = start;
        let (x1, y1) = end;

        let dx = (x1 as i32 - x0 as i32).abs();
        let dy = (y1 as i32 - y0 as i32).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = x0 as i32;
        let mut y = y0 as i32;

        loop {
            fb.write_pixel(x as usize, y as usize, color);

            if x == x1 as i32 && y == y1 as i32 {
                break;
            }

            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    });
}

pub fn draw_rect_outline(top_left: (usize, usize), bottom_right: (usize, usize), color: Color) {
    FRAMEBUFFER.get().map(|fb| {
        let mut fb = fb.lock();
        let (x0, y0) = top_left;
        let (x1, y1) = bottom_right;

        for x in x0..=x1 {
            fb.write_pixel(x, y0, color);
            fb.write_pixel(x, y1, color);
        }
        for y in y0..=y1 {
            fb.write_pixel(x0, y, color);
            fb.write_pixel(x1, y, color);
        }
    });
}

pub fn draw_rect(top_left: (usize, usize), bottom_right: (usize, usize), color: Color) {
    FRAMEBUFFER.get().map(|fb| {
        let mut fb = fb.lock();
        let (x0, y0) = top_left;
        let (x1, y1) = bottom_right;

        for x in x0..=x1 {
            for y in y0..=y1 {
                fb.write_pixel(x, y, color);
            }
        }
    });
}

pub fn draw_circle(center: (usize, usize), radius: usize, color: Color) {
    serial_println!("Drawing circle at {:?}", center);
    FRAMEBUFFER.get().map(|fb| {
        let mut fb = fb.lock();
        let (cx, cy) = center;
        let r_squared = (radius * radius) as i32;

        for y in 0..=radius {
            for x in 0..=radius {
                if (x * x + y * y) as i32 <= r_squared {
                    fb.write_pixel(cx + x, cy + y, color);
                    fb.write_pixel(cx - x, cy + y, color);
                    fb.write_pixel(cx + x, cy - y, color);
                    fb.write_pixel(cx - x, cy - y, color);
                }
            }
        }
    });
}

pub fn draw_circle_outline(center: (usize, usize), radius: usize, color: Color) {
    FRAMEBUFFER.get().map(|fb| {
        let mut fb = fb.lock();
        let (cx, cy) = center;
        let r_squared = (radius * radius) as i32;

        for y in 0..=radius {
            for x in 0..=radius {
                let distance_squared = (x * x + y * y) as i32;
                if distance_squared >= (radius.saturating_sub(1) * radius.saturating_sub(1)) as i32
                    && distance_squared <= r_squared
                {
                    fb.write_pixel(cx + x, cy + y, color);
                    fb.write_pixel(cx - x, cy + y, color);
                    fb.write_pixel(cx + x, cy - y, color);
                    fb.write_pixel(cx - x, cy - y, color);
                }
            }
        }
    });
}

pub fn clear_screen() {
    FRAMEBUFFER.get().map(|fb| fb.lock().clear());
}
