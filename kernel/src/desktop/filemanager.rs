use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use pc_keyboard::KeyCode;

use crate::{
    framebuffer::Color,
    fs::{
        fat32::FileEntry,
        manager::{create_file_in_root, delete_file_from_root, list_root_files},
    },
    serial_println,
    surface::{Shape, Surface},
};
use noto_sans_mono_bitmap::{FontWeight, RasterHeight};

const FILE_LIST_HEIGHT: usize = 280;
const FILE_ENTRY_HEIGHT: usize = 20;
const BUTTON_HEIGHT: usize = 25;
const MARGIN: usize = 10;
const TEXT_INPUT_HEIGHT: usize = 25;

#[derive(Clone, Debug)]
pub enum FileManagerMode {
    Browse,
    NewFile,
    DeleteFile,
    ViewFile(FileEntry),
}

pub struct FileManager {
    mode: FileManagerMode,
    files: Vec<FileEntry>,
    selected_file_index: Option<usize>,
    scroll_offset: usize,
    input_text: String,
    status_message: String,
    open_file_options: Option<Vec<(usize, String)>>, // Y offset, name
    selected_open_file_app: Option<String>,

    // UI element indices
    status_text_idx: Option<usize>,
    input_text_idx: Option<usize>,

    // Button indices
    new_file_btn_idx: Option<usize>,
    delete_file_btn_idx: Option<usize>,
    view_file_btn_idx: Option<usize>,
    back_btn_idx: Option<usize>,
    create_btn_idx: Option<usize>,
    confirm_delete_btn_idx: Option<usize>,
    confirm_open_file_btn_idx: Option<usize>,
}

impl FileManager {
    pub fn new() -> Self {
        let mut fm = Self {
            mode: FileManagerMode::Browse,
            files: Vec::new(),
            selected_file_index: None,
            scroll_offset: 0,
            input_text: String::new(),
            status_message: "Ready".to_string(),
            open_file_options: None,
            selected_open_file_app: None,

            status_text_idx: None,
            input_text_idx: None,

            new_file_btn_idx: None,
            delete_file_btn_idx: None,
            view_file_btn_idx: None,
            back_btn_idx: None,
            create_btn_idx: None,
            confirm_delete_btn_idx: None,
            confirm_open_file_btn_idx: None,
        };

        fm.refresh_file_list();
        fm
    }

    fn load_recomended_open_list(
        &self,
        file_name: &String,
    ) -> (Option<&'static str>, Vec<&'static str>) {
        let recomended = if file_name.to_lowercase().ends_with(".txt") {
            Some("notepad")
        } else {
            None
        };

        let other: Vec<&'static str> = if let Some(rec) = recomended {
            match rec {
                "notepad" => ["calculator"].to_vec(),
                _ => ["notepad", "calculator"].to_vec(),
            }
        } else {
            ["notepad", "calculator"].to_vec()
        };

        (recomended, other)
    }

    fn refresh_file_list(&mut self) {
        match list_root_files() {
            Ok(files) => {
                self.files = files.into_iter().filter(|f| !f.is_directory).collect();
                self.status_message = format!("Found {} files", self.files.len());
                serial_println!("File Manager: Found {} files", self.files.len());
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
                serial_println!("File Manager: Error listing files: {}", e);
            }
        }
    }

    pub fn setup_ui(&mut self, surface: &mut Surface) {
        self.clear_ui(surface);

        match &self.mode {
            FileManagerMode::Browse => self.setup_browse_ui(surface),
            FileManagerMode::NewFile => self.setup_new_file_ui(surface),
            FileManagerMode::DeleteFile => self.setup_delete_file_ui(surface),
            FileManagerMode::ViewFile(_) => self.setup_view_file_ui(surface),
        }
    }

    fn clear_ui(&mut self, surface: &mut Surface) {
        surface.clear_all_shapes();

        self.status_text_idx = None;
        self.input_text_idx = None;
        self.open_file_options = None;

        self.new_file_btn_idx = None;
        self.delete_file_btn_idx = None;
        self.view_file_btn_idx = None;
        self.back_btn_idx = None;
        self.create_btn_idx = None;
        self.confirm_delete_btn_idx = None;
        self.confirm_open_file_btn_idx = None;
    }

    fn setup_browse_ui(&mut self, surface: &mut Surface) {
        let width = surface.width;
        let height = surface.height;

        // File list background
        surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: 40,
            width: width - 2 * MARGIN,
            height: FILE_LIST_HEIGHT,
            color: Color::WHITE,
            filled: true,
            hide: false,
        });

        // File list border
        surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: 40,
            width: width - 2 * MARGIN,
            height: FILE_LIST_HEIGHT,
            color: Color::BLACK,
            filled: false,
            hide: false,
        });

        // Display files
        let max_visible_files = FILE_LIST_HEIGHT / FILE_ENTRY_HEIGHT;
        // let end_idx = (self.scroll_offset + max_visible_files).min(self.files.len());

        for (i, file) in self
            .files
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(max_visible_files)
        {
            let y_pos = 45 + (i - self.scroll_offset) * FILE_ENTRY_HEIGHT;
            let bg_color = if Some(i) == self.selected_file_index {
                Color::new(150, 200, 255)
            } else {
                Color::WHITE
            };

            // File entry background
            surface.add_shape(Shape::Rectangle {
                x: MARGIN + 2,
                y: y_pos,
                width: width - 2 * MARGIN - 4,
                height: FILE_ENTRY_HEIGHT - 2,
                color: bg_color,
                filled: true,
                hide: false,
            });

            // File name
            let display_name = if file.name.len() > 35 {
                format!("{}...", &file.name[..32])
            } else {
                file.name.clone()
            };

            surface.add_shape(Shape::Text {
                x: MARGIN + 5,
                y: y_pos + 3,
                content: display_name,
                color: Color::BLACK,
                background_color: bg_color,
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Regular,
                hide: false,
            });

            // File size
            let size_text = if file.size < 1024 {
                format!("{} B", file.size)
            } else if file.size < 1024 * 1024 {
                format!("{} KB", file.size / 1024)
            } else {
                format!("{} MB", file.size / (1024 * 1024))
            };

            surface.add_shape(Shape::Text {
                x: width - 80,
                y: y_pos + 3,
                content: size_text,
                color: Color::BLACK,
                background_color: bg_color,
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Regular,
                hide: false,
            });
        }

        // Buttons
        let button_y = height - 60;

        // New File button
        self.new_file_btn_idx = Some(surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::new(220, 220, 220),
            filled: true,
            hide: false,
        }));

        surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::BLACK,
            filled: false,
            hide: false,
        });

        surface.add_shape(Shape::Text {
            x: MARGIN + 10,
            y: button_y + 5,
            content: "New File".to_string(),
            color: Color::BLACK,
            background_color: Color::new(220, 220, 220),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // Delete File button
        self.delete_file_btn_idx = Some(surface.add_shape(Shape::Rectangle {
            x: MARGIN + 90,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::new(255, 180, 180),
            filled: true,
            hide: false,
        }));

        surface.add_shape(Shape::Rectangle {
            x: MARGIN + 90,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::BLACK,
            filled: false,
            hide: false,
        });

        surface.add_shape(Shape::Text {
            x: MARGIN + 100,
            y: button_y + 5,
            content: "Delete".to_string(),
            color: Color::BLACK,
            background_color: Color::new(255, 180, 180),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // View File button
        self.view_file_btn_idx = Some(surface.add_shape(Shape::Rectangle {
            x: MARGIN + 180,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::new(180, 255, 180),
            filled: true,
            hide: false,
        }));

        surface.add_shape(Shape::Rectangle {
            x: MARGIN + 180,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::BLACK,
            filled: false,
            hide: false,
        });

        surface.add_shape(Shape::Text {
            x: MARGIN + 200,
            y: button_y + 5,
            content: "Open".to_string(),
            color: Color::BLACK,
            background_color: Color::new(180, 255, 180),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // Status bar
        self.status_text_idx = Some(surface.add_shape(Shape::Text {
            x: MARGIN,
            y: height - 25,
            content: self.status_message.clone(),
            color: Color::BLACK,
            background_color: Color::new(240, 240, 240),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
    }

    fn setup_new_file_ui(&mut self, surface: &mut Surface) {
        let width = surface.width;
        let height = surface.height;

        // Title
        surface.add_shape(Shape::Text {
            x: MARGIN,
            y: 50,
            content: "Create New File".to_string(),
            color: Color::BLACK,
            background_color: Color::new(240, 240, 240),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Bold,
            hide: false,
        });

        // Filename input label
        surface.add_shape(Shape::Text {
            x: MARGIN,
            y: 80,
            content: "Filename:".to_string(),
            color: Color::BLACK,
            background_color: Color::new(240, 240, 240),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // Filename input background
        surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: 100,
            width: width - 2 * MARGIN,
            height: TEXT_INPUT_HEIGHT,
            color: Color::WHITE,
            filled: true,
            hide: false,
        });

        surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: 100,
            width: width - 2 * MARGIN,
            height: TEXT_INPUT_HEIGHT,
            color: Color::BLACK,
            filled: false,
            hide: false,
        });

        // Filename input text
        self.input_text_idx = Some(surface.add_shape(Shape::Text {
            x: MARGIN + 5,
            y: 105,
            content: format!("{}_", self.input_text),
            color: Color::BLACK,
            background_color: Color::WHITE,
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));

        // Buttons
        let button_y = height - 60;

        // Create button
        self.create_btn_idx = Some(surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::new(180, 255, 180),
            filled: true,
            hide: false,
        }));

        surface.add_shape(Shape::Rectangle {
            x: MARGIN,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::BLACK,
            filled: false,
            hide: false,
        });

        surface.add_shape(Shape::Text {
            x: MARGIN + 20,
            y: button_y + 5,
            content: "Create".to_string(),
            color: Color::BLACK,
            background_color: Color::new(180, 255, 180),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // Back button
        self.back_btn_idx = Some(surface.add_shape(Shape::Rectangle {
            x: MARGIN + 90,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::new(220, 220, 220),
            filled: true,
            hide: false,
        }));

        surface.add_shape(Shape::Rectangle {
            x: MARGIN + 90,
            y: button_y,
            width: 80,
            height: BUTTON_HEIGHT,
            color: Color::BLACK,
            filled: false,
            hide: false,
        });

        surface.add_shape(Shape::Text {
            x: MARGIN + 115,
            y: button_y + 5,
            content: "Back".to_string(),
            color: Color::BLACK,
            background_color: Color::new(220, 220, 220),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        });

        // Status
        self.status_text_idx = Some(surface.add_shape(Shape::Text {
            x: MARGIN,
            y: height - 25,
            content: "Enter filename and content, then click Create".to_string(),
            color: Color::BLACK,
            background_color: Color::new(240, 240, 240),
            font_size: RasterHeight::Size16,
            font_weight: FontWeight::Regular,
            hide: false,
        }));
    }

    fn setup_delete_file_ui(&mut self, surface: &mut Surface) {
        let height = surface.height;

        if let Some(idx) = self.selected_file_index {
            if let Some(file) = self.files.get(idx) {
                // Title
                surface.add_shape(Shape::Text {
                    x: MARGIN,
                    y: 50,
                    content: "Delete File".to_string(),
                    color: Color::BLACK,
                    background_color: Color::new(240, 240, 240),
                    font_size: RasterHeight::Size16,
                    font_weight: FontWeight::Bold,
                    hide: false,
                });

                // Confirmation message
                surface.add_shape(Shape::Text {
                    x: MARGIN,
                    y: 100,
                    content: format!("Are you sure you want to delete '{}'?", file.name),
                    color: Color::BLACK,
                    background_color: Color::new(240, 240, 240),
                    font_size: RasterHeight::Size16,
                    font_weight: FontWeight::Regular,
                    hide: false,
                });

                surface.add_shape(Shape::Text {
                    x: MARGIN,
                    y: 130,
                    content: "This action cannot be undone!".to_string(),
                    color: Color::new(200, 0, 0),
                    background_color: Color::new(240, 240, 240),
                    font_size: RasterHeight::Size16,
                    font_weight: FontWeight::Bold,
                    hide: false,
                });

                // Buttons
                let button_y = height - 60;

                // Confirm Delete button
                self.confirm_delete_btn_idx = Some(surface.add_shape(Shape::Rectangle {
                    x: MARGIN,
                    y: button_y,
                    width: 100,
                    height: BUTTON_HEIGHT,
                    color: Color::new(255, 100, 100),
                    filled: true,
                    hide: false,
                }));

                surface.add_shape(Shape::Rectangle {
                    x: MARGIN,
                    y: button_y,
                    width: 100,
                    height: BUTTON_HEIGHT,
                    color: Color::BLACK,
                    filled: false,
                    hide: false,
                });

                surface.add_shape(Shape::Text {
                    x: MARGIN + 15,
                    y: button_y + 5,
                    content: "Yes, Delete".to_string(),
                    color: Color::BLACK,
                    background_color: Color::new(255, 100, 100),
                    font_size: RasterHeight::Size16,
                    font_weight: FontWeight::Regular,
                    hide: false,
                });

                // Back button
                self.back_btn_idx = Some(surface.add_shape(Shape::Rectangle {
                    x: MARGIN + 110,
                    y: button_y,
                    width: 80,
                    height: BUTTON_HEIGHT,
                    color: Color::new(220, 220, 220),
                    filled: true,
                    hide: false,
                }));

                surface.add_shape(Shape::Rectangle {
                    x: MARGIN + 110,
                    y: button_y,
                    width: 80,
                    height: BUTTON_HEIGHT,
                    color: Color::BLACK,
                    filled: false,
                    hide: false,
                });

                surface.add_shape(Shape::Text {
                    x: MARGIN + 135,
                    y: button_y + 5,
                    content: "Cancel".to_string(),
                    color: Color::BLACK,
                    background_color: Color::new(220, 220, 220),
                    font_size: RasterHeight::Size16,
                    font_weight: FontWeight::Regular,
                    hide: false,
                });
            }
        }
    }

    fn setup_view_file_ui(&mut self, surface: &mut Surface) {
        let height = surface.height;

        if let FileManagerMode::ViewFile(file) = &self.mode {
            // Title
            surface.add_shape(Shape::Text {
                x: MARGIN,
                y: 50,
                content: format!("Select an application to open: {}", file.name),
                color: Color::BLACK,
                background_color: Color::new(240, 240, 240),
                font_size: RasterHeight::Size20,
                font_weight: FontWeight::Bold,
                hide: false,
            });

            surface.add_shape(Shape::Text {
                x: MARGIN,
                y: 70,
                content: "Recomended:".to_string(),
                color: Color::BLACK,
                background_color: Color::new(240, 240, 240),
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Bold,
                hide: false,
            });

            let (recommended, all) = self.load_recomended_open_list(&file.name);

            if recommended.is_some() && self.selected_open_file_app.is_none() {
                self.selected_open_file_app = recommended.map(|s| s.to_string());
            }

            if recommended.is_some()
                && self.selected_open_file_app == recommended.map(|s| s.to_string())
            {
                surface.add_shape(Shape::Rectangle {
                    x: MARGIN,
                    y: 90,
                    width: 200,
                    height: 20,
                    color: Color::new(150, 200, 255),
                    filled: true,
                    hide: false,
                });
            }

            self.open_file_options = Some(Vec::new());
            if recommended.is_some() {
                self.open_file_options
                    .as_mut()
                    .unwrap()
                    .push((90, recommended.unwrap().to_string()));
            }

            surface.add_shape(Shape::Text {
                x: MARGIN,
                y: 90,
                content: recommended
                    .unwrap_or("No recommended apps found")
                    .to_string(),
                color: Color::BLACK,
                background_color: Color::new(240, 240, 240),
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Regular,
                hide: false,
            });

            surface.add_shape(Shape::Text {
                x: MARGIN,
                y: 110,
                content: "Other:".to_string(),
                color: Color::BLACK,
                background_color: Color::new(240, 240, 240),
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Bold,
                hide: false,
            });

            for (i, app) in all.iter().enumerate() {
                if self.selected_open_file_app == Some(app.to_string()) {
                    surface.add_shape(Shape::Rectangle {
                        x: MARGIN,
                        y: 130 + i * 20,
                        width: 200,
                        height: 20,
                        color: Color::new(150, 200, 255),
                        filled: true,
                        hide: false,
                    });
                }

                surface.add_shape(Shape::Text {
                    x: MARGIN,
                    y: 130 + i * 20,
                    content: app.to_string(),
                    color: Color::BLACK,
                    background_color: Color::new(240, 240, 240),
                    font_size: RasterHeight::Size16,
                    font_weight: FontWeight::Regular,
                    hide: false,
                });

                self.open_file_options
                    .as_mut()
                    .unwrap()
                    .push((130 + i * 20, app.to_string()));
            }

            // Back button
            let button_y = height - 60;
            self.back_btn_idx = Some(surface.add_shape(Shape::Rectangle {
                x: MARGIN,
                y: button_y,
                width: 80,
                height: BUTTON_HEIGHT,
                color: Color::new(220, 220, 220),
                filled: true,
                hide: false,
            }));

            surface.add_shape(Shape::Rectangle {
                x: MARGIN,
                y: button_y,
                width: 80,
                height: BUTTON_HEIGHT,
                color: Color::BLACK,
                filled: false,
                hide: false,
            });

            surface.add_shape(Shape::Text {
                x: MARGIN + 25,
                y: button_y + 5,
                content: "Back".to_string(),
                color: Color::BLACK,
                background_color: Color::new(220, 220, 220),
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Regular,
                hide: false,
            });

            self.confirm_open_file_btn_idx = Some(surface.add_shape(Shape::Rectangle {
                x: MARGIN + 90,
                y: button_y,
                width: 80,
                height: BUTTON_HEIGHT,
                color: Color::new(180, 255, 180),
                filled: true,
                hide: false,
            }));

            surface.add_shape(Shape::Rectangle {
                x: MARGIN + 90,
                y: button_y,
                width: 80,
                height: BUTTON_HEIGHT,
                color: Color::BLACK,
                filled: false,
                hide: false,
            });

            surface.add_shape(Shape::Text {
                x: MARGIN + 115,
                y: button_y + 5,
                content: "Open".to_string(),
                color: Color::BLACK,
                background_color: Color::new(180, 255, 180),
                font_size: RasterHeight::Size16,
                font_weight: FontWeight::Regular,
                hide: false,
            });
        }
    }

    pub fn handle_click(
        &mut self,
        x: usize,
        y: usize,
        surface: &mut Surface,
    ) -> (bool, Option<(FileEntry, String)>) {
        match &self.mode {
            FileManagerMode::Browse => (self.handle_browse_click(x, y, surface), None),
            FileManagerMode::NewFile => (self.handle_new_file_click(x, y, surface), None),
            FileManagerMode::DeleteFile => (self.handle_delete_click(x, y, surface), None),
            FileManagerMode::ViewFile(_) => self.handle_view_click(x, y, surface),
        }
    }

    fn handle_browse_click(&mut self, x: usize, y: usize, surface: &mut Surface) -> bool {
        // Check file list clicks
        if x >= MARGIN && x < surface.width - MARGIN && y >= 45 && y < 45 + FILE_LIST_HEIGHT {
            let clicked_index = self.scroll_offset + (y - 45) / FILE_ENTRY_HEIGHT;
            if clicked_index < self.files.len() {
                self.selected_file_index = Some(clicked_index);
                self.setup_ui(surface);
                return true;
            }
        }

        // Check button clicks
        if self.new_file_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN, surface.height - 60, 80, BUTTON_HEIGHT) {
                self.mode = FileManagerMode::NewFile;
                self.input_text.clear();
                self.setup_ui(surface);
                return true;
            }
        }

        if self.delete_file_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN + 90, surface.height - 60, 80, BUTTON_HEIGHT) {
                if self.selected_file_index.is_some() {
                    self.mode = FileManagerMode::DeleteFile;
                    self.setup_ui(surface);
                } else {
                    self.status_message = "Please select a file to delete".to_string();
                    self.setup_ui(surface);
                }
                return true;
            }
        }

        if self.view_file_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN + 180, surface.height - 60, 80, BUTTON_HEIGHT) {
                if let Some(idx) = self.selected_file_index {
                    if let Some(file) = self.files.get(idx).cloned() {
                        self.mode = FileManagerMode::ViewFile(file);
                        self.setup_ui(surface);
                    }
                } else {
                    self.status_message = "Please select a file to view".to_string();
                    self.setup_ui(surface);
                }
                return true;
            }
        }

        false
    }

    fn handle_new_file_click(&mut self, x: usize, y: usize, surface: &mut Surface) -> bool {
        if self.create_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN, surface.height - 60, 80, BUTTON_HEIGHT) {
                self.create_file(surface);
                return true;
            }
        }

        if self.back_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN + 90, surface.height - 60, 80, BUTTON_HEIGHT) {
                self.mode = FileManagerMode::Browse;
                self.setup_ui(surface);
                return true;
            }
        }

        false
    }

    fn handle_delete_click(&mut self, x: usize, y: usize, surface: &mut Surface) -> bool {
        if self.confirm_delete_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN, surface.height - 60, 100, BUTTON_HEIGHT) {
                self.delete_selected_file(surface);
                return true;
            }
        }

        if self.back_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN + 110, surface.height - 60, 80, BUTTON_HEIGHT) {
                self.mode = FileManagerMode::Browse;
                self.setup_ui(surface);
                return true;
            }
        }

        false
    }

    fn handle_view_click(
        &mut self,
        x: usize,
        y: usize,
        surface: &mut Surface,
    ) -> (bool, Option<(FileEntry, String)>) {
        if self.back_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN, surface.height - 60, 80, BUTTON_HEIGHT) {
                self.mode = FileManagerMode::Browse;
                self.setup_ui(surface);

                return (true, None);
            }
        }

        if self.confirm_open_file_btn_idx.is_some() {
            if self.is_button_clicked(x, y, MARGIN + 90, surface.height - 60, 80, BUTTON_HEIGHT) {
                if let Some(app) = self.selected_open_file_app.clone() {
                    let file = self
                        .files
                        .get(self.selected_file_index.unwrap())
                        .cloned()
                        .unwrap();

                    self.selected_open_file_app = None;
                    self.mode = FileManagerMode::Browse;
                    self.setup_ui(surface);

                    return (true, Some((file, app)));
                } else {
                    self.status_message =
                        "Please select an application to open the file".to_string();
                    self.setup_ui(surface);
                }
                return (true, None);
            }
        }

        if let Some(apps) = &self.open_file_options {
            for (app_y, app) in apps {
                if self.is_button_clicked(x, y, MARGIN, *app_y, 200, 20) {
                    self.selected_open_file_app = Some(app.to_string());
                    self.setup_ui(surface);
                    return (true, None);
                }
            }
        }

        (false, None)
    }

    fn is_button_clicked(
        &self,
        x: usize,
        y: usize,
        btn_x: usize,
        btn_y: usize,
        btn_width: usize,
        btn_height: usize,
    ) -> bool {
        x >= btn_x && x < btn_x + btn_width && y >= btn_y && y < btn_y + btn_height
    }

    fn create_file(&mut self, surface: &mut Surface) {
        if self.input_text.is_empty() {
            self.status_message = "Please enter a filename".to_string();
            if let Some(idx) = self.status_text_idx {
                surface.update_text_content(idx, self.status_message.clone(), None);
            }
            return;
        }

        match create_file_in_root(&self.input_text, &[]) {
            Ok(_) => {
                self.status_message = format!("File '{}' created successfully", self.input_text);
                self.refresh_file_list();
                self.mode = FileManagerMode::Browse;
                self.setup_ui(surface);
            }
            Err(e) => {
                self.status_message = format!("Error creating file: {}", e);
                if let Some(idx) = self.status_text_idx {
                    surface.update_text_content(idx, self.status_message.clone(), None);
                }
            }
        }
    }

    fn delete_selected_file(&mut self, surface: &mut Surface) {
        if let Some(idx) = self.selected_file_index {
            if let Some(file) = self.files.get(idx) {
                let filename = file.name.clone();
                match delete_file_from_root(&filename) {
                    Ok(_) => {
                        self.status_message = format!("File '{}' deleted successfully", filename);
                        self.refresh_file_list();
                        self.selected_file_index = None;
                        self.mode = FileManagerMode::Browse;
                        self.setup_ui(surface);
                    }
                    Err(e) => {
                        self.status_message = format!("Error deleting file: {}", e);
                        self.mode = FileManagerMode::Browse;
                        self.setup_ui(surface);
                    }
                }
            }
        }
    }

    pub fn handle_char_input(&mut self, c: char, surface: &mut Surface) {
        match &self.mode {
            FileManagerMode::NewFile => {
                if c == '\x08' {
                    // Backspace
                    self.input_text.pop();
                } else if c == '\n' {
                    // Enter key, create file
                    self.create_file(surface);
                } else if c.is_ascii() && !c.is_control() {
                    self.input_text.push(c);
                }

                if let Some(idx) = self.input_text_idx {
                    surface.update_text_content(idx, format!("{}_", self.input_text), None);
                }
            }
            _ => {}
        }
    }

    pub fn handle_key_input(&mut self, key: KeyCode, surface: &mut Surface) {
        match &self.mode {
            FileManagerMode::NewFile => match key {
                KeyCode::Backspace => {
                    self.input_text.pop();
                    if let Some(idx) = self.input_text_idx {
                        surface.update_text_content(idx, format!("{}_", self.input_text), None);
                    }
                }
                _ => {}
            },
            FileManagerMode::Browse => match key {
                KeyCode::ArrowUp => {
                    if let Some(ref mut idx) = self.selected_file_index {
                        if *idx > 0 {
                            *idx -= 1;
                            self.setup_ui(surface);
                        }
                    } else if !self.files.is_empty() {
                        self.selected_file_index = Some(self.files.len() - 1);
                        self.setup_ui(surface);
                    }
                }
                KeyCode::ArrowDown => {
                    if let Some(ref mut idx) = self.selected_file_index {
                        if *idx < self.files.len() - 1 {
                            *idx += 1;
                            self.setup_ui(surface);
                        }
                    } else if !self.files.is_empty() {
                        self.selected_file_index = Some(0);
                        self.setup_ui(surface);
                    }
                }
                KeyCode::Return => {
                    if let Some(idx) = self.selected_file_index {
                        if let Some(file) = self.files.get(idx).cloned() {
                            self.mode = FileManagerMode::ViewFile(file);
                            self.setup_ui(surface);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn render(&mut self, _surface: &mut Surface) {
        // The UI is already set up, just make sure it's current
        // This could be extended to handle dynamic updates
    }
}
