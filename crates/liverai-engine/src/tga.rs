//! TGA output in the form iRacing loads: 24-bit true colour, run-length encoded.
//!
//! The layout matches what Pillow writes for the proof-of-concept paints: image type 10,
//! bottom-left origin, BGR pixel order, packets that never cross a scanline, and a
//! TGA 2.0 footer.

use std::io::{self, Write};

/// TGA image type for RLE true-colour images.
pub const IMAGE_TYPE_RLE_TRUECOLOR: u8 = 10;

const FOOTER: &[u8] = b"\0\0\0\0\0\0\0\0TRUEVISION-XFILE.\0";

/// The header fields the lint checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TgaHeader {
    pub image_type: u8,
    pub width: u16,
    pub height: u16,
    pub bits_per_pixel: u8,
    pub top_left_origin: bool,
}

/// Encode top-down RGB8 pixels as a 24-bit RLE TGA.
pub fn encode_rgb_rle(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), (width * height * 3) as usize, "RGB buffer size");
    assert!(
        width <= u16::MAX as u32 && height <= u16::MAX as u32,
        "TGA size limit"
    );
    let (w, h) = (width as usize, height as usize);
    let mut out = Vec::with_capacity(18 + rgb.len() / 2 + FOOTER.len());
    out.extend_from_slice(&[0, 0, IMAGE_TYPE_RLE_TRUECOLOR, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&(width as u16).to_le_bytes());
    out.extend_from_slice(&(height as u16).to_le_bytes());
    out.push(24);
    out.push(0); // bottom-left origin, no alpha bits

    for y in (0..h).rev() {
        let row = &rgb[y * w * 3..(y + 1) * w * 3];
        let px = |x: usize| -> [u8; 3] { [row[x * 3 + 2], row[x * 3 + 1], row[x * 3]] };
        let mut x = 0;
        while x < w {
            let p = px(x);
            let mut run = 1;
            while x + run < w && run < 128 && px(x + run) == p {
                run += 1;
            }
            if run > 1 {
                out.push(0x80 | (run as u8 - 1));
                out.extend_from_slice(&p);
                x += run;
            } else {
                // Raw packet: extend until a run of two identical pixels starts.
                let start = x;
                let mut len = 1;
                while start + len < w && len < 128 {
                    let next_run = start + len + 1 < w && px(start + len) == px(start + len + 1);
                    if next_run {
                        break;
                    }
                    len += 1;
                }
                out.push(len as u8 - 1);
                for i in start..start + len {
                    out.extend_from_slice(&px(i));
                }
                x += len;
            }
        }
    }
    out.extend_from_slice(FOOTER);
    out
}

/// Write top-down RGB8 pixels as a 24-bit RLE TGA.
pub fn write_rgb_rle(mut w: impl Write, width: u32, height: u32, rgb: &[u8]) -> io::Result<()> {
    w.write_all(&encode_rgb_rle(width, height, rgb))
}

/// Read the header fields of a TGA file.
pub fn read_header(bytes: &[u8]) -> Option<TgaHeader> {
    if bytes.len() < 18 {
        return None;
    }
    Some(TgaHeader {
        image_type: bytes[2],
        width: u16::from_le_bytes([bytes[12], bytes[13]]),
        height: u16::from_le_bytes([bytes[14], bytes[15]]),
        bits_per_pixel: bytes[16],
        top_left_origin: bytes[17] & 0x20 != 0,
    })
}

/// Decode a 24-bit RLE TGA written by [`encode_rgb_rle`] back to top-down RGB8.
/// Used by tests and by the lint to confirm a file round-trips.
pub fn decode_rgb_rle(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let header = read_header(bytes)?;
    if header.image_type != IMAGE_TYPE_RLE_TRUECOLOR || header.bits_per_pixel != 24 {
        return None;
    }
    let (w, h) = (header.width as usize, header.height as usize);
    let id_len = bytes[0] as usize;
    let mut pos = 18 + id_len;
    let mut bgr = Vec::with_capacity(w * h * 3);
    while bgr.len() < w * h * 3 {
        let head = *bytes.get(pos)?;
        pos += 1;
        let count = (head & 0x7F) as usize + 1;
        if head & 0x80 != 0 {
            let p = bytes.get(pos..pos + 3)?;
            pos += 3;
            for _ in 0..count {
                bgr.extend_from_slice(p);
            }
        } else {
            bgr.extend_from_slice(bytes.get(pos..pos + count * 3)?);
            pos += count * 3;
        }
    }
    if bgr.len() != w * h * 3 {
        return None;
    }
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        let src_y = if header.top_left_origin { y } else { h - 1 - y };
        for x in 0..w {
            let s = (src_y * w + x) * 3;
            let d = (y * w + x) * 3;
            rgb[d] = bgr[s + 2];
            rgb[d + 1] = bgr[s + 1];
            rgb[d + 2] = bgr[s];
        }
    }
    Some((w as u32, h as u32, rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(w: u32, h: u32) -> Vec<u8> {
        let mut rgb = Vec::new();
        for y in 0..h {
            for x in 0..w {
                // Mix of long runs, short runs and noise.
                let v = if y % 3 == 0 {
                    [255, 95, 31]
                } else if x % 5 < 2 {
                    [14, 165, 233]
                } else {
                    [
                        (x * 7 % 256) as u8,
                        (y * 13 % 256) as u8,
                        ((x ^ y) % 256) as u8,
                    ]
                };
                rgb.extend_from_slice(&v);
            }
        }
        rgb
    }

    #[test]
    fn round_trips_and_has_iracing_header() {
        for (w, h) in [(1, 1), (2, 3), (129, 5), (300, 7), (256, 256)] {
            let rgb = sample(w, h);
            let bytes = encode_rgb_rle(w, h, &rgb);
            let header = read_header(&bytes).unwrap();
            assert_eq!(header.image_type, 10);
            assert_eq!(header.bits_per_pixel, 24);
            assert_eq!((header.width as u32, header.height as u32), (w, h));
            assert!(!header.top_left_origin);
            assert!(bytes.ends_with(b"TRUEVISION-XFILE.\0"));
            let (dw, dh, decoded) = decode_rgb_rle(&bytes).unwrap();
            assert_eq!((dw, dh), (w, h));
            assert_eq!(decoded, rgb, "{w}x{h}");
        }
    }

    #[test]
    fn flat_image_compresses() {
        let rgb = vec![5u8; 512 * 512 * 3];
        let bytes = encode_rgb_rle(512, 512, &rgb);
        assert!(bytes.len() < 512 * 512 * 3 / 20);
    }

    #[test]
    fn encoding_is_deterministic() {
        let rgb = sample(200, 50);
        assert_eq!(encode_rgb_rle(200, 50, &rgb), encode_rgb_rle(200, 50, &rgb));
    }
}
