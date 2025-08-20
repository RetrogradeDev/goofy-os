use alloc::{format, string::ToString, vec::Vec};
use noto_sans_mono_bitmap::{FontWeight, RasterHeight};

use crate::{
    framebuffer::Color,
    surface::{Shape, Surface},
    sysinfo::{SystemInfo, estimate_stack_usage, format_memory_size},
};

pub struct SysInfo {
    system_info: SystemInfo,
    text_lines: Vec<usize>, // Shape indices for text lines
    previous_stack_usage: usize,
    refresh_button_region: (usize, usize, usize, usize), // (x, y, width, height)
}

impl SysInfo {
    pub fn new() -> Self {
        Self {
            system_info: SystemInfo::gather(),
            text_lines: Vec::new(),
            previous_stack_usage: 0,
            refresh_button_region: (0, 0, 0, 0),
        }
    }

    pub fn init(&mut self, surface: &mut Surface) {
        let mut y_offset = 20;
        let line_height = 18;
        let x_start = 15;

        // Title
        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: "SYSTEM INFORMATION".to_string(),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size20,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height + 5;

        // Separator
        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: "--------------".to_string(),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Light,
            hide: false,
        }));
        y_offset += line_height;

        // OS Information
        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: format!(
                "OS: {} {}",
                self.system_info.os_name, self.system_info.os_version
            ),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height;

        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: format!("Architecture: {}", self.system_info.architecture),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height;

        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: format!("Processor: {}", self.system_info.processor_model),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height + 5;

        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: format!("Processor vendor: {}", self.system_info.processor_vendor),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height + 5;

        self.text_lines.push(
            surface.add_shape(Shape::Text {
                x: x_start,
                y: y_offset,
                content: format!(
                    "Base Frequency: {}",
                    self.system_info
                        .base_frequency
                        .map(|f| format!("{} MHz", f))
                        .unwrap_or("Unknown".to_string())
                ),
                color: Color::WHITE,
                background_color: Color::DARKGRAY,
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Regular,
                hide: false,
            }),
        );
        y_offset += line_height + 5;

        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: format!(
                    "Max Frequency: {}",
                    self.system_info
                        .max_frequency
                        .map(|f| format!("{} MHz", f))
                        .unwrap_or("Unknown".to_string())
                ),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height + 5;

        // Memory Information
        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: "MEMORY USAGE".to_string(),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height;

        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: format!(
                "Heap: {} / {}",
                format_memory_size(self.system_info.heap_used),
                format_memory_size(self.system_info.heap_size)
            ),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height;

        let stack_usage = estimate_stack_usage();
        self.previous_stack_usage = stack_usage;
        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: format!(
                "Stack: {} / {}",
                format_memory_size(stack_usage),
                format_memory_size(self.system_info.stack_size)
            ),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height;

        // CPU Features
        self.text_lines.push(surface.add_shape(Shape::Text {
            x: x_start,
            y: y_offset,
            content: "CPU FEATURES".to_string(),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
        y_offset += line_height;

        // Show CPU features (split into lines if too many)
        let features_text = if self.system_info.cpu_features.is_empty() {
            "No features detected".to_string()
        } else {
            self.system_info.cpu_features.join(", ")
        };

        // Split long feature list into multiple lines
        let max_chars_per_line = 40;
        let mut remaining = features_text.as_str();
        while !remaining.is_empty() {
            let chunk = if remaining.len() <= max_chars_per_line {
                remaining
            } else {
                // Find a good split point (comma or space)
                let mut split_pos = max_chars_per_line;
                if let Some(pos) = remaining[..max_chars_per_line].rfind(", ") {
                    split_pos = pos + 2;
                } else if let Some(pos) = remaining[..max_chars_per_line].rfind(' ') {
                    split_pos = pos + 1;
                }
                &remaining[..split_pos]
            };

            self.text_lines.push(surface.add_shape(Shape::Text {
                x: x_start,
                y: y_offset,
                content: chunk.to_string(),
                color: Color::WHITE,
                background_color: Color::DARKGRAY,
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Regular,
                hide: false,
            }));
            y_offset += line_height;

            remaining = &remaining[chunk.len()..];
        }

        y_offset += 10;

        // Refresh button
        self.refresh_button_region = (x_start, y_offset, 100, 25);
        surface.add_shape(Shape::Rectangle {
            x: self.refresh_button_region.0,
            y: self.refresh_button_region.1,
            width: self.refresh_button_region.2,
            height: self.refresh_button_region.3,
            color: Color::new(200, 200, 255),
            filled: true,
            hide: false,
        });

        surface.add_shape(Shape::Text {
            x: self.refresh_button_region.0 + 20,
            y: self.refresh_button_region.1 + 5,
            content: "Refresh".to_string(),
            color: Color::BLACK,
            background_color: Color::new(200, 200, 255),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // Info text
        surface.add_shape(Shape::Text {
            x: x_start + 120,
            y: y_offset + 8,
            content: "Updates stack usage".to_string(),
            color: Color::WHITE,
            background_color: Color::DARKGRAY,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Light,
            hide: false,
        });
    }

    pub fn handle_mouse_click(&mut self, x: usize, y: usize) {
        // Check if click is on refresh button
        if x >= self.refresh_button_region.0
            && x < self.refresh_button_region.0 + self.refresh_button_region.2
            && y >= self.refresh_button_region.1
            && y < self.refresh_button_region.1 + self.refresh_button_region.3
        {
            self.refresh_data();
        }
    }

    fn refresh_data(&mut self) {
        // Update system information (mainly dynamic data like stack usage)
        self.system_info = SystemInfo::gather();
    }

    pub fn render(&mut self, surface: &mut Surface) {
        // Update stack usage if it changed significantly
        let current_stack_usage = estimate_stack_usage();
        if (current_stack_usage as i32 - self.previous_stack_usage as i32).abs() > 100 {
            self.previous_stack_usage = current_stack_usage;

            if self.text_lines.len() > 10 {
                let stack_text = format!(
                    "Stack: {} / {}",
                    format_memory_size(current_stack_usage),
                    format_memory_size(self.system_info.stack_size)
                );
                surface.update_text_content(self.text_lines[10], stack_text, None);
            }
        }
    }
}
