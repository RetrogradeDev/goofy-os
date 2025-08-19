use alloc::{string::String, vec, vec::Vec};
use noto_sans_mono_bitmap::{FontWeight, RasterHeight};

use crate::framebuffer::{Color, FrameBufferWriter};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width).max(other.x + other.width);
        let y2 = (self.y + self.height).max(other.y + other.height);

        Rect {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        }
    }

    pub fn contains_point(&self, x: usize, y: usize) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

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
        background_color: Color,

        font_size: RasterHeight,
        font_weight: FontWeight,

        hide: bool,
    },
}

impl Shape {
    pub fn get_bounds(&self) -> Rect {
        match self {
            Shape::Rectangle {
                x,
                y,
                width,
                height,
                ..
            } => Rect {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            },
            Shape::Text { x, y, content, .. } => {
                // Approximate text bounds
                let char_width = 8; // Approximate character width
                let char_height = 16; // Approximate character height
                Rect {
                    x: *x,
                    y: *y,
                    width: content.len() * char_width,
                    height: char_height,
                }
            }
        }
    }

    pub fn intersects_rect(&self, rect: &Rect) -> bool {
        if let Shape::Rectangle { hide: true, .. } | Shape::Text { hide: true, .. } = self {
            return false;
        }

        self.get_bounds().intersects(rect)
    }

    pub fn set_position(&mut self, x: usize, y: usize) -> Rect {
        let old_bounds = self.get_bounds();

        match self {
            Shape::Rectangle {
                x: shape_x,
                y: shape_y,
                ..
            } => {
                *shape_x = x;
                *shape_y = y;
            }
            Shape::Text {
                x: shape_x,
                y: shape_y,
                ..
            } => {
                *shape_x = x;
                *shape_y = y;
            }
        }

        old_bounds.union(&self.get_bounds())
    }

    pub fn set_visibility(&mut self, visible: bool) -> Rect {
        let bounds = self.get_bounds();
        match self {
            Shape::Rectangle { hide, .. } => {
                *hide = !visible;
            }
            Shape::Text { hide, .. } => {
                *hide = !visible;
            }
        }
        bounds
    }

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
                background_color,
                font_size,
                font_weight,
                hide,
            } => {
                if *hide {
                    return;
                }

                framebuffer.draw_raw_text(
                    content,
                    *x + offset_x,
                    *y + offset_y,
                    *color,
                    *background_color,
                    *font_weight,
                    *font_size,
                );
            }
        }
    }

    pub fn render_clipped(
        &self,
        framebuffer: &mut FrameBufferWriter,
        offset_x: usize,
        offset_y: usize,
        clip_rect: &Rect,
    ) {
        let shape_bounds = self.get_bounds();
        if !shape_bounds.intersects(clip_rect) {
            return;
        }

        // For now, just use regular render - clipping can be optimized later
        self.render(framebuffer, offset_x, offset_y);
    }
}

pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub background_color: Color,
    pub just_fill_bg: bool,
    shapes: Vec<Shape>,
    pub is_dirty: bool,
    pub dirty_regions: Vec<Rect>,
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
            dirty_regions: vec![Rect::new(0, 0, width, height)], // Initially everything is dirty
        }
    }

    pub fn mark_region_dirty(&mut self, region: Rect) {
        let expanded_region = self.expand_region_for_overlapping_shapes(region);

        // Merge overlapping dirty regions to avoid fragmentation
        let mut merged = false;
        for existing in &mut self.dirty_regions {
            if existing.intersects(&expanded_region) {
                *existing = existing.union(&expanded_region);
                merged = true;
                break;
            }
        }

        if !merged {
            self.dirty_regions.push(expanded_region);
        }

        self.is_dirty = true;
    }

    /// Expand a dirty region to include all shapes that overlap with it (recursively)
    fn expand_region_for_overlapping_shapes(&self, initial_region: Rect) -> Rect {
        let mut current_region = initial_region;
        let mut changed = true;

        // Keep expanding until no more overlapping shapes are found
        while changed {
            changed = false;
            let previous_region = current_region;

            // Find all shapes that intersect with the current region
            for shape in &self.shapes {
                if shape.intersects_rect(&current_region) {
                    let shape_bounds = shape.get_bounds();
                    let new_region = current_region.union(&shape_bounds);

                    // If the region expanded, we need to check again for more overlaps
                    if new_region.width != current_region.width
                        || new_region.height != current_region.height
                        || new_region.x != current_region.x
                        || new_region.y != current_region.y
                    {
                        current_region = new_region;
                        changed = true;
                    }
                }
            }

            // Safety check to prevent infinite loops (shouldn't happen but just in case)
            if current_region == previous_region {
                break;
            }
        }

        current_region
    }

    pub fn force_dirty_region(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.mark_region_dirty(Rect::new(x, y, width, height));
    }

    pub fn force_full_redraw(&mut self) {
        self.dirty_regions.clear();
        self.dirty_regions
            .push(Rect::new(0, 0, self.width, self.height));
        self.is_dirty = true;
    }

    pub fn add_shape(&mut self, shape: Shape) -> usize {
        let bounds = shape.get_bounds();
        self.shapes.push(shape);
        self.mark_region_dirty(bounds);
        self.shapes.len() - 1
    }

    // Shape modification methods with automatic dirty tracking
    pub fn move_shape(&mut self, shape_id: usize, new_x: usize, new_y: usize) -> bool {
        if let Some(shape) = self.shapes.get_mut(shape_id) {
            let dirty_region = shape.set_position(new_x, new_y);
            self.mark_region_dirty(dirty_region);
            true
        } else {
            false
        }
    }

    pub fn hide_shape(&mut self, shape_id: usize) -> bool {
        if let Some(shape) = self.shapes.get_mut(shape_id) {
            let dirty_region = shape.set_visibility(false);
            self.mark_region_dirty(dirty_region);
            true
        } else {
            false
        }
    }

    pub fn show_shape(&mut self, shape_id: usize) -> bool {
        if let Some(shape) = self.shapes.get_mut(shape_id) {
            let dirty_region = shape.set_visibility(true);
            self.mark_region_dirty(dirty_region);
            true
        } else {
            false
        }
    }

    pub fn update_rectangle_size(
        &mut self,
        shape_id: usize,
        new_width: usize,
        new_height: usize,
    ) -> bool {
        if let Some(Shape::Rectangle { width, height, .. }) = self.shapes.get_mut(shape_id) {
            let old_bounds = Rect {
                x: 0,
                y: 0,
                width: *width,
                height: *height,
            }; // We'll get proper bounds after
            *width = new_width;
            *height = new_height;

            // Get the actual shape bounds now
            let shape_bounds = self.shapes[shape_id].get_bounds();
            let dirty_bounds = Rect {
                x: shape_bounds.x,
                y: shape_bounds.y,
                width: old_bounds.width.max(new_width),
                height: old_bounds.height.max(new_height),
            };
            self.mark_region_dirty(dirty_bounds);
            true
        } else {
            false
        }
    }

    pub fn update_rectangle_color(&mut self, shape_id: usize, new_color: Color) -> bool {
        if let Some(Shape::Rectangle { color, .. }) = self.shapes.get_mut(shape_id) {
            *color = new_color;
            let bounds = self.shapes[shape_id].get_bounds();
            self.mark_region_dirty(bounds);
            true
        } else {
            false
        }
    }

    pub fn update_rectangle_filled(&mut self, shape_id: usize, filled: bool) -> bool {
        if let Some(Shape::Rectangle {
            filled: shape_filled,
            ..
        }) = self.shapes.get_mut(shape_id)
        {
            *shape_filled = filled;
            let bounds = self.shapes[shape_id].get_bounds();
            self.mark_region_dirty(bounds);
            true
        } else {
            false
        }
    }

    pub fn update_text_content(&mut self, shape_id: usize, new_content: String) -> bool {
        if let Some(Shape::Text { content, x, y, .. }) = self.shapes.get_mut(shape_id) {
            // Calculate old bounds using current position and old content length
            let old_width = content.len() * 8; // char_width
            let old_bounds = Rect::new(*x, *y, old_width, 16); // char_height = 16

            *content = new_content;

            // Get new bounds AFTER changing content
            let new_bounds = self.shapes[shape_id].get_bounds();

            // Mark the union of old and new bounds as dirty
            let dirty_bounds = old_bounds.union(&new_bounds);

            self.mark_region_dirty(dirty_bounds);
            true
        } else {
            false
        }
    }

    pub fn update_text_color(&mut self, shape_id: usize, new_color: Color) -> bool {
        if let Some(Shape::Text { color, .. }) = self.shapes.get_mut(shape_id) {
            *color = new_color;
            let bounds = self.shapes[shape_id].get_bounds();
            self.mark_region_dirty(bounds);
            true
        } else {
            false
        }
    }

    pub fn remove_shape(&mut self, shape_id: usize) -> bool {
        if shape_id < self.shapes.len() {
            let bounds = self.shapes[shape_id].get_bounds();
            self.shapes.remove(shape_id);
            self.mark_region_dirty(bounds);
            true
        } else {
            false
        }
    }

    pub fn get_shape_bounds(&self, shape_id: usize) -> Option<Rect> {
        self.shapes.get(shape_id).map(|shape| shape.get_bounds())
    }

    pub fn is_shape_visible(&self, shape_id: usize) -> Option<bool> {
        self.shapes.get(shape_id).map(|shape| match shape {
            Shape::Rectangle { hide, .. } | Shape::Text { hide, .. } => !hide,
        })
    }

    pub fn clear_all_shapes(&mut self) {
        if !self.shapes.is_empty() {
            self.shapes.clear();
            self.mark_region_dirty(Rect::new(0, 0, self.width, self.height));
        }
    }

    pub fn get_shapes_at_point(&self, x: usize, y: usize) -> Vec<usize> {
        let mut result = Vec::new();
        for (i, shape) in self.shapes.iter().enumerate() {
            if shape.get_bounds().contains_point(x, y) {
                if let Shape::Rectangle { hide: false, .. } | Shape::Text { hide: false, .. } =
                    shape
                {
                    result.push(i);
                }
            }
        }
        result
    }

    pub fn render(
        &mut self,
        framebuffer: &mut FrameBufferWriter,
        offset_x: usize,
        offset_y: usize,
        force: bool,
    ) -> bool {
        if !self.is_dirty && !force {
            return false;
        }

        if force {
            self.dirty_regions.clear();
            self.dirty_regions
                .push(Rect::new(0, 0, self.width, self.height));
        }

        // Render each dirty region
        for region in &self.dirty_regions {
            // Clear the dirty region with background
            // Use just_fill_bg mode only if we're doing a full redraw OR if there are no shapes
            if self.just_fill_bg && (force || self.shapes.is_empty()) {
                // For just_fill_bg mode with full redraw or no shapes
                for y in region.y..(region.y + region.height) {
                    for x in region.x..(region.x + region.width) {
                        if x < self.width && y < self.height {
                            framebuffer.write_pixel(
                                x + offset_x,
                                y + offset_y,
                                self.background_color,
                            );
                        }
                    }
                }
            } else {
                // Use rect-based clearing for dirty regions with shapes
                framebuffer.draw_rect(
                    (region.x + offset_x, region.y + offset_y),
                    (
                        region.x + region.width - 1 + offset_x,
                        region.y + region.height - 1 + offset_y,
                    ),
                    self.background_color,
                );
            }

            // Only render shapes that intersect with this dirty region
            for shape in &self.shapes {
                if shape.intersects_rect(region) {
                    shape.render_clipped(framebuffer, offset_x, offset_y, region);
                }
            }
        }

        self.dirty_regions.clear();
        self.is_dirty = false;
        true
    }

    /// Get the dirty regions without clearing them (for checking intersections)
    pub fn get_dirty_regions(&self) -> &[Rect] {
        &self.dirty_regions
    }

    /// Check if any dirty regions intersect with the given rectangle
    pub fn intersects_dirty_regions(&self, rect: &Rect) -> bool {
        if !self.is_dirty {
            return false;
        }

        self.dirty_regions
            .iter()
            .any(|dirty_rect| dirty_rect.intersects(rect))
    }

    /// Get the surface bounds as a Rect
    pub fn get_bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }
}
