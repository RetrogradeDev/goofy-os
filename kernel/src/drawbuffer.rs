use alloc::vec::Vec;

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    #[inline(always)]
    fn blend_over(self, dst: Color) -> Color {
        // Alpha blending: out = src + dst*(1 - src.a)
        let sa = self.a as u32;
        let da = dst.a as u32;
        let inv_sa = 255 - sa;
        let out_a = sa + da * inv_sa / 255;
        let out_r = (self.r as u32 * sa + dst.r as u32 * inv_sa) / 255;
        let out_g = (self.g as u32 * sa + dst.g as u32 * inv_sa) / 255;
        let out_b = (self.b as u32 * sa + dst.b as u32 * inv_sa) / 255;
        Color {
            r: out_r as u8,
            g: out_g as u8,
            b: out_b as u8,
            a: out_a as u8,
        }
    }
}

/// A 2D pixel surface with RGBA8 pixels.
pub struct Surface {
    width: u32,
    height: u32,
    buf: Vec<u8>, // len = width*height*4
}

impl Surface {
    /// Create a new blank surface (all pixels = fully transparent).
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .expect("Surface dimensions too large");
        Surface {
            width,
            height,
            buf: alloc::vec![0; len],
        }
    }

    /// Fill the entire surface with a single color.
    pub fn fill(&mut self, color: Color) {
        let pattern = [color.r, color.g, color.b, color.a];
        let len = self.buf.len();
        let mut i = 0;

        // Write one row of color manually
        let row = self.width as usize;
        let mut pattern_row = Vec::with_capacity(row * 4);
        for _ in 0..row {
            pattern_row.extend_from_slice(&pattern);
        }

        // Copy that pattern_row into every line
        while i < len {
            self.buf[i..i + row * 4].copy_from_slice(&pattern_row);
            i += row * 4;
        }
    }

    /// Replace the pixel at (x,y) with `color`. Panics on OOB.
    #[inline(always)]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        debug_assert!(x < self.width && y < self.height);
        let idx = ((y * self.width + x) << 2) as usize;
        let pix = &mut self.buf[idx..idx + 4];
        pix[0] = color.r;
        pix[1] = color.g;
        pix[2] = color.b;
        pix[3] = color.a;
    }

    /// Blend `color` over the pixel at (x,y) using its alpha channel.
    #[inline(always)]
    pub fn set_pixel_with_opacity(&mut self, x: u32, y: u32, color: Color) {
        debug_assert!(x < self.width && y < self.height);
        let idx = ((y * self.width + x) << 2) as usize;
        // Load destination
        let dst = Color {
            r: self.buf[idx],
            g: self.buf[idx + 1],
            b: self.buf[idx + 2],
            a: self.buf[idx + 3],
        };
        // Blend
        let out = color.blend_over(dst);
        // Store
        self.buf[idx] = out.r;
        self.buf[idx + 1] = out.g;
        self.buf[idx + 2] = out.b;
        self.buf[idx + 3] = out.a;
    }

    /// Blit another surface at offset (ox,oy), replacing pixels.
    pub fn insert_surface(&mut self, ox: u32, oy: u32, src: &Surface) {
        // Determine copyable region
        let max_w = (self.width).saturating_sub(ox);
        let max_h = (self.height).saturating_sub(oy);
        let w = max_w.min(src.width);
        let h = max_h.min(src.height);
        let dst_stride = (self.width << 2) as usize;
        let src_stride = (src.width << 2) as usize;
        let row_bytes = (w << 2) as usize;

        let mut dst_offset = ((oy * self.width + ox) << 2) as usize;
        let mut src_offset = 0usize;

        for _ in 0..h {
            unsafe {
                // Row-wise copy: src.buf[src_offset..src_offset+row_bytes]
                //           to self.buf[dst_offset..dst_offset+row_bytes]
                core::ptr::copy_nonoverlapping(
                    src.buf.as_ptr().add(src_offset),
                    self.buf.as_mut_ptr().add(dst_offset),
                    row_bytes,
                );
            }
            dst_offset += dst_stride;
            src_offset += src_stride;
        }
    }

    /// Blit another surface at (ox,oy), blending with per-pixel alpha.
    pub fn insert_surface_with_opacity(&mut self, ox: u32, oy: u32, src: &Surface) {
        let max_w = (self.width).saturating_sub(ox);
        let max_h = (self.height).saturating_sub(oy);
        let w = max_w.min(src.width);
        let h = max_h.min(src.height);

        for row in 0..h {
            for col in 0..w {
                let sx = col;
                let sy = row;
                let idx_src = ((sy * src.width + sx) << 2) as usize;
                let color = Color {
                    r: src.buf[idx_src],
                    g: src.buf[idx_src + 1],
                    b: src.buf[idx_src + 2],
                    a: src.buf[idx_src + 3],
                };
                if color.a == 0 {
                    continue;
                }
                let dx = ox + col;
                let dy = oy + row;
                self.set_pixel_with_opacity(dx, dy, color);
            }
        }
    }

    /// Get a reference to the raw RGBA buffer.
    pub fn data(&self) -> &[u8] {
        &self.buf
    }

    /// Get mutable reference to raw buffer.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    /// Dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
