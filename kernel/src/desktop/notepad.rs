use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use noto_sans_mono_bitmap::{FontWeight, RasterHeight};
use pc_keyboard::KeyCode;

use crate::{
    framebuffer::Color,
    fs::{fat32::FileEntry, manager::read_text_file},
    serial_println,
    surface::{Shape, Surface},
};

pub struct Notepad {
    text_content: String,
    cursor_position: usize,
    scroll_offset: usize,
    display_lines: Vec<String>,
    text_area_idx: usize,
    cursor_idx: usize,
    max_chars_per_line: usize,
    max_visible_lines: usize,
    previous_content: String,
    prev_cursor_x: usize,
    prev_cursor_y: usize,
    open_file: Option<FileEntry>,
}

impl Notepad {
    pub fn new(mut file_entry: Option<FileEntry>) -> Self {
        let text_content = if let Some(ref file) = file_entry {
            match read_text_file(file.first_cluster, file.size) {
                Ok(content) => content,
                Err(error) => {
                    serial_println!("Failed to open file {}", file.name);
                    serial_println!("Error: {}", error);

                    file_entry = None;

                    String::new()
                }
            }
        } else {
            String::new()
        };

        Self {
            text_content,
            cursor_position: 0,
            scroll_offset: 0,
            display_lines: Vec::new(),
            text_area_idx: 0,
            cursor_idx: 0,
            max_chars_per_line: 84, // Approximate characters that fit in the text area
            max_visible_lines: 22,  // Number of lines visible in the text area
            previous_content: String::new(),
            prev_cursor_x: 0,
            prev_cursor_y: 0,
            open_file: file_entry,
        }
    }

    pub fn init(&mut self, surface: &mut Surface) {
        // Text content display
        self.text_area_idx = surface.add_shape(Shape::Text {
            x: 5,
            y: 5,
            content: self.get_display_text(),
            color: Color::BLACK,
            background_color: Color::WHITE,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // Cursor (simple vertical line)
        self.cursor_idx = surface.add_shape(Shape::Rectangle {
            x: 5,
            y: 5,
            width: 1,
            height: 16,
            color: Color::BLACK,
            filled: true,
            hide: false,
        });

        self.update_display_lines();
    }

    pub fn handle_char_input(&mut self, ch: char) {
        match ch {
            '\u{08}' => {
                // Backspace
                if self.cursor_position > 0 {
                    self.text_content.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            '\r' | '\n' => {
                // Enter - add newline
                self.text_content.insert(self.cursor_position, '\n');
                self.cursor_position += 1;
            }
            ch if ch.is_control() => {
                // Ignore other control characters
            }
            _ => {
                // Regular character
                self.text_content.insert(self.cursor_position, ch);
                self.cursor_position += 1;
            }
        }

        self.update_display_lines();
        self.update_scroll_if_needed();
    }

    pub fn handle_key_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::ArrowLeft => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::ArrowRight => {
                if self.cursor_position < self.text_content.len() {
                    self.cursor_position += 1;
                }
            }
            _ => {}
        }
    }

    fn update_display_lines(&mut self) {
        self.display_lines.clear();

        // Split text into lines and wrap long lines
        let lines: Vec<&str> = self.text_content.split('\n').collect();

        for line in lines {
            if line.len() <= self.max_chars_per_line {
                self.display_lines.push(line.to_string());
            } else {
                // Wrap long lines
                let mut remaining = line;
                while remaining.len() > self.max_chars_per_line {
                    let (chunk, rest) = remaining.split_at(self.max_chars_per_line);
                    self.display_lines.push(chunk.to_string());
                    remaining = rest;
                }
                if !remaining.is_empty() {
                    self.display_lines.push(remaining.to_string());
                }
            }
        }
    }

    fn update_scroll_if_needed(&mut self) {
        // Calculate which line the cursor is on
        let mut char_count = 0;
        let mut cursor_line = 0;

        for (line_idx, line) in self.display_lines.iter().enumerate() {
            if char_count + line.len() + 1 > self.cursor_position {
                cursor_line = line_idx;
                break;
            }
            char_count += line.len() + 1; // +1 for newline
        }

        // Adjust scroll if cursor is outside visible area
        if cursor_line < self.scroll_offset {
            self.scroll_offset = cursor_line;
        } else if cursor_line >= self.scroll_offset + self.max_visible_lines {
            self.scroll_offset = cursor_line - self.max_visible_lines + 1;
        }
    }

    fn get_display_text(&self) -> String {
        let visible_lines: Vec<String> = self
            .display_lines
            .iter()
            .skip(self.scroll_offset)
            .take(self.max_visible_lines)
            .cloned()
            .collect();

        visible_lines.join("\n")
    }

    fn get_cursor_visual_position(&self) -> (usize, usize) {
        // Calculate cursor position relative to the visible text area
        let mut char_count = 0;
        let mut line_in_visible = 0;
        let mut col_in_line = 0;

        for (line_idx, line) in self.display_lines.iter().enumerate() {
            if line_idx < self.scroll_offset {
                char_count += line.len() + 1;
                continue;
            }

            if char_count + line.len() + 1 > self.cursor_position {
                col_in_line = self.cursor_position - char_count;
                break;
            }

            char_count += line.len() + 1;
            line_in_visible += 1;

            if line_in_visible >= self.max_visible_lines {
                break;
            }
        }

        // Convert to pixel coordinates (approximate)
        let x = 3 + col_in_line * 7; // 8 pixels per character (approximate)
        let y = 5 + line_in_visible * 18; // 18 pixels per line (16 + spacing)

        (x, y)
    }

    pub fn render(&mut self, surface: &mut Surface) {
        let current_display = self.get_display_text();

        // Only update if content changed
        if current_display != self.previous_content {
            surface.update_text_content(self.text_area_idx, current_display.clone(), None);
            self.previous_content = current_display;
        }

        // Update cursor position
        let (cursor_x, cursor_y) = self.get_cursor_visual_position();
        if cursor_x != self.prev_cursor_x || cursor_y != self.prev_cursor_y {
            surface.move_shape(self.cursor_idx, cursor_x, cursor_y);

            self.prev_cursor_x = cursor_x;
            self.prev_cursor_y = cursor_y;
        }
    }
}
