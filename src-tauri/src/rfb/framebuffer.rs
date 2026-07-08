//! Server-side copy of the remote framebuffer.
//!
//! The webview canvas is the user's *view*; this is the source of truth. We
//! keep it so the Agent Bridge can produce a `screenshot` without round-tripping
//! through the webview (which may be scaled, occluded, or — for a headless
//! agent call — not even the focus). Every decoded draw op is applied here in
//! the same order it's sent to the canvas, so the two never diverge.

use super::Error;

pub struct Framebuffer {
    pub width: u16,
    pub height: u16,
    /// RGBA, row-major, width*height*4 bytes.
    rgba: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            rgba: vec![0u8; width as usize * height as usize * 4],
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.rgba = vec![0u8; width as usize * height as usize * 4];
    }

    /// Copy a decoded RGBA rect in, clipping to bounds defensively.
    pub fn blit(&mut self, x: u16, y: u16, w: u16, h: u16, src: &[u8]) {
        let fbw = self.width as usize;
        let fbh = self.height as usize;
        let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
        if src.len() < w * h * 4 {
            return;
        }
        for row in 0..h {
            let dy = y + row;
            if dy >= fbh {
                break;
            }
            let cols = w.min(fbw.saturating_sub(x));
            if cols == 0 {
                continue;
            }
            let dst = (dy * fbw + x) * 4;
            let s = row * w * 4;
            self.rgba[dst..dst + cols * 4].copy_from_slice(&src[s..s + cols * 4]);
        }
    }

    /// Apply a CopyRect: move a region within the framebuffer. Snapshots the
    /// source first, so overlapping src/dst is always correct.
    pub fn copy_rect(&mut self, x: u16, y: u16, w: u16, h: u16, src_x: u16, src_y: u16) {
        let fbw = self.width as usize;
        let fbh = self.height as usize;
        let (w, h) = (w as usize, h as usize);
        let (sx, sy) = (src_x as usize, src_y as usize);
        let mut tmp = vec![0u8; w * h * 4];
        for row in 0..h {
            let syr = sy + row;
            if syr >= fbh {
                break;
            }
            let cols = w.min(fbw.saturating_sub(sx));
            if cols == 0 {
                continue;
            }
            let s = (syr * fbw + sx) * 4;
            let d = row * w * 4;
            tmp[d..d + cols * 4].copy_from_slice(&self.rgba[s..s + cols * 4]);
        }
        self.blit(x, y, w as u16, h as u16, &tmp);
    }

    pub fn to_png(&self) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width as u32, self.height as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
            writer
                .write_image_data(&self.rgba)
                .map_err(|e| format!("png data: {e}"))?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blit_then_copyrect_moves_pixels() {
        let mut fb = Framebuffer::new(4, 4);
        // paint a red pixel at (0,0)
        fb.blit(0, 0, 1, 1, &[255, 0, 0, 255]);
        // copy the top-left 1x1 to (3,3)
        fb.copy_rect(3, 3, 1, 1, 0, 0);
        let png = fb.to_png().unwrap();
        assert_eq!(&png[1..4], b"PNG");
        // pixel (3,3) is the last pixel: offset (3*4+3)*4 = 60
        assert_eq!(&fb.rgba[60..64], &[255, 0, 0, 255]);
    }

    #[test]
    fn blit_clips_out_of_bounds() {
        let mut fb = Framebuffer::new(2, 2);
        // a 2x2 rect placed at (1,1) should only write the bottom-right pixel
        fb.blit(
            1,
            1,
            2,
            2,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        );
        assert_eq!(&fb.rgba[12..16], &[1, 2, 3, 4]); // (1,1) got src row 0 col 0
    }
}
