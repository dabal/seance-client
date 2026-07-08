//! ZRLE (encoding 16) tile decoder. Operates on an already-inflated byte
//! stream (the per-session zlib stream lives in the read loop, since ZRLE
//! shares one zlib dictionary across all rects of a connection).
//!
//! CPIXEL note: we negotiate 32bpp/depth-24 little-endian with shifts
//! 16/8/0, so all color bits live in the 3 least significant bytes and the
//! wire carries 3-byte compressed pixels ordered B,G,R.

use super::Error;

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Result<u8, Error> {
        let b = *self.data.get(self.pos).ok_or("ZRLE data truncated")?;
        self.pos += 1;
        Ok(b)
    }

    fn cpixel(&mut self) -> Result<[u8; 4], Error> {
        if self.pos + 3 > self.data.len() {
            return Err("ZRLE data truncated".into());
        }
        let b = &self.data[self.pos..self.pos + 3];
        self.pos += 3;
        Ok([b[2], b[1], b[0], 255]) // wire is B,G,R -> RGBA
    }
}

fn put(out: &mut [u8], stride: usize, x: usize, y: usize, px: [u8; 4]) {
    let off = (y * stride + x) * 4;
    out[off..off + 4].copy_from_slice(&px);
}

/// Run length: one or more bytes; 255 means "add 255 and keep reading";
/// total = sum + 1.
fn run_len(c: &mut Cursor) -> Result<usize, Error> {
    let mut len = 1usize;
    loop {
        let b = c.u8()?;
        len += b as usize;
        if b != 255 {
            return Ok(len);
        }
    }
}

/// Decode a whole ZRLE rect (inflated bytes) into an RGBA buffer of w*h*4.
pub fn decode(data: &[u8], w: usize, h: usize) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; w * h * 4];
    let mut c = Cursor { data, pos: 0 };
    let mut ty = 0;
    while ty < h {
        let th = (h - ty).min(64);
        let mut tx = 0;
        while tx < w {
            let tw = (w - tx).min(64);
            decode_tile(&mut c, &mut out, w, tx, ty, tw, th)?;
            tx += 64;
        }
        ty += 64;
    }
    Ok(out)
}

fn read_palette(c: &mut Cursor, size: usize) -> Result<Vec<[u8; 4]>, Error> {
    (0..size).map(|_| c.cpixel()).collect()
}

fn decode_tile(
    c: &mut Cursor,
    out: &mut [u8],
    stride: usize,
    tx: usize,
    ty: usize,
    tw: usize,
    th: usize,
) -> Result<(), Error> {
    let sub = c.u8()?;
    match sub {
        // raw
        0 => {
            for y in 0..th {
                for x in 0..tw {
                    let px = c.cpixel()?;
                    put(out, stride, tx + x, ty + y, px);
                }
            }
        }
        // solid color
        1 => {
            let px = c.cpixel()?;
            for y in 0..th {
                for x in 0..tw {
                    put(out, stride, tx + x, ty + y, px);
                }
            }
        }
        // packed palette
        2..=16 => {
            let palette = read_palette(c, sub as usize)?;
            let bits: u32 = match palette.len() {
                2 => 1,
                3..=4 => 2,
                _ => 4,
            };
            for y in 0..th {
                // each row starts at a byte boundary
                let mut byte = 0u8;
                let mut nbits = 0u32;
                for x in 0..tw {
                    if nbits == 0 {
                        byte = c.u8()?;
                        nbits = 8;
                    }
                    nbits -= bits;
                    let idx = ((byte >> nbits) & ((1u8 << bits) - 1)) as usize;
                    let px = *palette.get(idx).ok_or("ZRLE palette index out of range")?;
                    put(out, stride, tx + x, ty + y, px);
                }
            }
        }
        // plain RLE
        128 => {
            let total = tw * th;
            let mut i = 0;
            while i < total {
                let px = c.cpixel()?;
                let run = run_len(c)?;
                if i + run > total {
                    return Err("ZRLE run overflows tile".into());
                }
                for _ in 0..run {
                    put(out, stride, tx + i % tw, ty + i / tw, px);
                    i += 1;
                }
            }
        }
        // palette RLE
        130..=255 => {
            let palette = read_palette(c, (sub - 128) as usize)?;
            let total = tw * th;
            let mut i = 0;
            while i < total {
                let b = c.u8()?;
                let (idx, run) = if b & 0x80 != 0 {
                    ((b & 0x7f) as usize, run_len(c)?)
                } else {
                    (b as usize, 1)
                };
                let px = *palette.get(idx).ok_or("ZRLE palette index out of range")?;
                if i + run > total {
                    return Err("ZRLE run overflows tile".into());
                }
                for _ in 0..run {
                    put(out, stride, tx + i % tw, ty + i / tw, px);
                    i += 1;
                }
            }
        }
        other => return Err(format!("invalid ZRLE subencoding {other}").into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::decode;

    // cpixel on the wire is B,G,R; decoded pixel is R,G,B,255
    const BLUE_CP: [u8; 3] = [255, 0, 0];
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const RED_CP: [u8; 3] = [0, 0, 255];
    const RED: [u8; 4] = [255, 0, 0, 255];

    fn px(out: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let off = (y * w + x) * 4;
        out[off..off + 4].try_into().unwrap()
    }

    #[test]
    fn solid_tile() {
        let mut data = vec![1u8];
        data.extend_from_slice(&BLUE_CP);
        let out = decode(&data, 3, 3).unwrap();
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(px(&out, 3, x, y), BLUE);
            }
        }
    }

    #[test]
    fn packed_palette_1bit() {
        // 10x1, palette of 2, alternating: 10 bits -> 0b10101010 0b10______
        let mut data = vec![2u8];
        data.extend_from_slice(&BLUE_CP);
        data.extend_from_slice(&RED_CP);
        data.extend_from_slice(&[0b10101010, 0b10000000]);
        let out = decode(&data, 10, 1).unwrap();
        for x in 0..10 {
            assert_eq!(px(&out, 10, x, 0), if x % 2 == 0 { RED } else { BLUE });
        }
    }

    #[test]
    fn plain_rle_spills_across_rows() {
        // 4x2, one run of 8 (length byte 7 => 1+7)
        let mut data = vec![128u8];
        data.extend_from_slice(&RED_CP);
        data.push(7);
        let out = decode(&data, 4, 2).unwrap();
        for y in 0..2 {
            for x in 0..4 {
                assert_eq!(px(&out, 4, x, y), RED);
            }
        }
    }

    #[test]
    fn palette_rle() {
        // 4x2: run of 4 x palette[0], run of 3 x palette[1], single palette[1]
        let mut data = vec![130u8];
        data.extend_from_slice(&BLUE_CP);
        data.extend_from_slice(&RED_CP);
        data.extend_from_slice(&[0x80, 3, 0x81, 2, 1]);
        let out = decode(&data, 4, 2).unwrap();
        for i in 0..8 {
            let expect = if i < 4 { BLUE } else { RED };
            assert_eq!(px(&out, 4, i % 4, i / 4), expect);
        }
    }

    #[test]
    fn run_overflow_rejected() {
        let mut data = vec![128u8];
        data.extend_from_slice(&RED_CP);
        data.push(200); // run of 201 in a 4x2 tile
        assert!(decode(&data, 4, 2).is_err());
    }
}
