use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use noto_sans_mono_bitmap::{FontWeight, RasterHeight};

use crate::{
    framebuffer::Color,
    surface::{Shape, Surface},
};

pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

pub enum CalculatorState {
    InputFirst,
    InputSecond(f64, Operation),
    Result(f64),
}

pub struct Calculator {
    state: CalculatorState,
    previous_display_text: String,
    display_text: String,
    current_input: String,
    display_idx: usize,
    button_regions: Vec<(usize, usize, usize, usize)>, // (x, y, width, height)
}

impl Calculator {
    pub fn new() -> Self {
        Self {
            state: CalculatorState::InputFirst,
            previous_display_text: String::new(),
            display_text: "0".to_string(),
            current_input: String::new(),
            display_idx: 0,
            button_regions: Vec::new(),
        }
    }

    pub fn init(&mut self, surface: &mut Surface) {
        let button_height: usize = 50;
        let button_width: usize = 40;
        let button_spacing: usize = 5;
        let start_x = 15;
        let start_y = 85;

        surface.add_shape(Shape::Rectangle {
            x: 15,
            y: 15,
            width: 175,
            height: 60,
            color: Color::WHITE,
            filled: true,
            hide: false,
        });

        self.display_idx = surface.add_shape(Shape::Text {
            x: 20,
            y: 30,
            content: self.display_text.clone(),
            color: Color::BLACK,
            background_color: Color::WHITE,
            font_size: RasterHeight::Size32,
            font_weight: FontWeight::Light,
            hide: false,
        });

        let buttons = [
            ["7", "8", "9", ":"],
            ["4", "5", "6", "x"],
            ["1", "2", "3", "-"],
            ["0", ".", "=", "+"],
        ];

        for (row, button_row) in buttons.iter().enumerate() {
            for (col, &button) in button_row.iter().enumerate() {
                let x = start_x + col * (button_width + button_spacing);
                let y = start_y + row * (button_height + button_spacing);
                self.button_regions
                    .push((x, y, button_width, button_height));

                surface.add_shape(Shape::Rectangle {
                    x,
                    y,
                    width: button_width,
                    height: button_height,
                    color: Color::WHITE,
                    filled: true,
                    hide: false,
                });
                surface.add_shape(Shape::Text {
                    x: x + 13,
                    y: y + 15,
                    content: button.to_string(),
                    color: Color::BLACK,
                    background_color: Color::WHITE,
                    font_size: RasterHeight::Size24,
                    font_weight: FontWeight::Light,
                    hide: false,
                });
            }
        }
    }

    pub fn handle_mouse_click(&mut self, x: usize, y: usize) {
        for (idx, &(button_x, button_y, width, height)) in self.button_regions.iter().enumerate() {
            if x >= button_x && x < button_x + width && y >= button_y && y < button_y + height {
                self.handle_button_click(idx);
                return;
            }
        }
    }

    fn handle_button_click(&mut self, idx: usize) {
        let label = match idx {
            0 => "7",
            1 => "8",
            2 => "9",
            3 => ":",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "x",
            8 => "1",
            9 => "2",
            10 => "3",
            11 => "-",
            12 => "0",
            13 => ".",
            14 => "=",
            15 => "+",
            _ => return, // Invalid index
        };

        match label {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                self.current_input.push_str(label);
                self.display_text = self.current_input.clone();
            }
            "." => {
                if !self.current_input.contains('.') {
                    self.current_input.push('.');
                    self.display_text = self.current_input.clone();
                }
            }
            "=" => {
                if let CalculatorState::InputSecond(first, op) = &self.state {
                    if let Ok(second) = self.current_input.parse::<f64>() {
                        let result = match op {
                            Operation::Add => first + second,
                            Operation::Subtract => first - second,
                            Operation::Multiply => first * second,
                            Operation::Divide => first / second,
                        };

                        self.state = CalculatorState::Result(result);

                        // Only 12 digits fit on the screen // TODO: fix this
                        self.display_text = format!("{:.10}", result);

                        self.current_input.clear();
                    }
                }
            }
            "+" => {
                if let Ok(first) = self.current_input.parse::<f64>() {
                    self.state = CalculatorState::InputSecond(first, Operation::Add);
                    self.current_input.clear();
                    self.display_text += " + ";
                }
            }
            "-" => {
                if let Ok(first) = self.current_input.parse::<f64>() {
                    self.state = CalculatorState::InputSecond(first, Operation::Subtract);
                    self.current_input.clear();
                    self.display_text += " - ";
                }
            }
            "x" => {
                if let Ok(first) = self.current_input.parse::<f64>() {
                    self.state = CalculatorState::InputSecond(first, Operation::Multiply);
                    self.current_input.clear();
                    self.display_text += " x ";
                }
            }
            ":" => {
                if let Ok(first) = self.current_input.parse::<f64>() {
                    self.state = CalculatorState::InputSecond(first, Operation::Divide);
                    self.current_input.clear();
                    self.display_text += " / ";
                }
            }
            _ => {}
        }
    }

    pub fn render(&mut self, surface: &mut Surface) {
        if self.display_text == self.previous_display_text {
            return; // No change in display text, nothing to update
        }

        surface.update_text_content(self.display_idx, self.display_text.clone(), None);

        self.previous_display_text = self.display_text.clone();
    }
}
