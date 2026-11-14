//! B0 calibration paint: every UV island in its own flat colour, stamped with its id and an
//! arrow toward the top of the canvas. Loaded in the Paint Shop, it shows which island is
//! which body panel and which way is up on each.

use std::path::Path;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use serde::Serialize;

use crate::islands::{calibration_colour, segment, IslandMeta, SegmentParams};
use crate::raster::Grid;
use crate::render::{paint_file_names, valid_paint_id};
use crate::template::{Role, Template};
use crate::{tga, Error, Result};

const FONT: &[u8] = include_bytes!("../assets/Roboto-Bold.ttf");
/// Cockpit Grey: the area outside the islands.
const BACKGROUND: [u8; 3] = [17, 24, 39];
const EDGE: [u8; 3] = [10, 10, 10];
/// Islands with a smaller inscribed radius get no label.
const MIN_LABEL_RADIUS: f64 = 9.0;

pub struct Calibration {
    pub width: u32,
    pub height: u32,
    /// RGB8, top-down.
    pub diffuse: Vec<u8>,
    /// The template's default spec map.
    pub spec: Vec<u8>,
    pub islands: Vec<IslandMeta>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CalibrationSummary {
    pub canvas: [u32; 2],
    pub island_count: usize,
    pub paintable_px: usize,
    pub files: Vec<String>,
}

/// Coverage buffer for anti-aliased drawing: 0.0 = untouched, 1.0 = fully covered.
struct Coverage {
    grid: Grid<f32>,
}

impl Coverage {
    fn new(w: usize, h: usize) -> Self {
        Self {
            grid: Grid::new(w, h, 0.0),
        }
    }

    fn add(&mut self, x: i64, y: i64, c: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.grid.w && (y as usize) < self.grid.h {
            let i = y as usize * self.grid.w + x as usize;
            self.grid.data[i] = self.grid.data[i].max(c);
        }
    }

    fn rect(&mut self, x0: i64, y0: i64, x1: i64, y1: i64) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.add(x, y, 1.0);
            }
        }
    }

    /// Upward-pointing triangle with its tip at `(cx, tip_y)`.
    fn arrow_head(&mut self, cx: i64, tip_y: i64, half_width: i64, height: i64) {
        for dy in 0..height {
            let half = (half_width * dy + height / 2) / height.max(1);
            self.rect(cx - half, tip_y + dy, cx + half + 1, tip_y + dy + 1);
        }
    }

    fn text_centered(&mut self, font: &FontRef, text: &str, px: f32, cx: f32, cy: f32) {
        let scaled = font.as_scaled(PxScale::from(px));
        let width: f32 = text
            .chars()
            .map(|c| scaled.h_advance(scaled.glyph_id(c)))
            .sum();
        let mut pen_x = cx - width / 2.0;
        // Centre the digits' cap height on `cy`.
        let baseline = cy + (scaled.ascent() + scaled.descent()) / 2.0;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            let glyph =
                id.with_scale_and_position(PxScale::from(px), ab_glyph::point(pen_x, baseline));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, c| {
                    self.add(
                        bounds.min.x as i64 + gx as i64,
                        bounds.min.y as i64 + gy as i64,
                        c,
                    );
                });
            }
            pen_x += scaled.h_advance(id);
        }
    }

    /// Grow the covered area by `r` pixels (for an outline behind the shape).
    fn grown(&self, r: i64) -> Coverage {
        let mut out = Coverage::new(self.grid.w, self.grid.h);
        for y in 0..self.grid.h as i64 {
            for x in 0..self.grid.w as i64 {
                let c = self.grid.data[y as usize * self.grid.w + x as usize];
                if c <= 0.0 {
                    continue;
                }
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy <= r * r {
                            out.add(x + dx, y + dy, c);
                        }
                    }
                }
            }
        }
        out
    }

    fn paint(&self, rgb: &mut [u8], x0: usize, y0: usize, canvas_w: usize, colour: [u8; 3]) {
        for y in 0..self.grid.h {
            for x in 0..self.grid.w {
                let c = self.grid.data[y * self.grid.w + x];
                if c <= 0.0 {
                    continue;
                }
                let i = ((y0 + y) * canvas_w + x0 + x) * 3;
                for k in 0..3 {
                    let dst = rgb[i + k] as f32;
                    rgb[i + k] = (colour[k] as f32 * c + dst * (1.0 - c)).round() as u8;
                }
            }
        }
    }
}

pub fn calibrate(template: &Template, params: &SegmentParams) -> Result<Calibration> {
    let (w, h) = (template.width() as usize, template.height() as usize);
    let paintable = template.paintable()?;
    let wire = template.role_rgba(Role::Wire)?;
    let islands = segment(&paintable, &wire, params);
    let edges = islands.edges();
    let meta = islands.meta();

    let mut diffuse = Vec::with_capacity(w * h * 3);
    for i in 0..w * h {
        let l = islands.labels.data[i];
        let colour = if edges.data[i] && paintable.data[i] {
            EDGE
        } else if l > 0 {
            calibration_colour(l)
        } else {
            BACKGROUND
        };
        diffuse.extend_from_slice(&colour);
    }

    let font =
        FontRef::try_from_slice(FONT).map_err(|e| Error::Invalid(format!("bundled font: {e}")))?;
    for island in &meta {
        let r = island.max_inscribed_r;
        if r < MIN_LABEL_RADIUS {
            continue;
        }
        let font_px = (r * 0.52).clamp(13.0, 46.0);
        let arrow_len = (r * 0.45).clamp(9.0, 38.0);
        // Draw into a local tile around the island's centre, then composite outline + fill.
        let pad = (font_px * 2.5 + arrow_len) as i64 + 8;
        let (cx, cy) = (island.center[0] as i64, island.center[1] as i64);
        let (tx0, ty0) = ((cx - pad).max(0), (cy - pad).max(0));
        let (tx1, ty1) = ((cx + pad).min(w as i64), (cy + pad).min(h as i64));
        let mut shape = Coverage::new((tx1 - tx0) as usize, (ty1 - ty0) as usize);
        let (lx, ly) = (cx - tx0, cy - ty0);

        let shaft_bottom = ly - (font_px * 0.55) as i64;
        let shaft_top = shaft_bottom - arrow_len as i64;
        let half = ((font_px / 20.0) as i64).max(1);
        shape.rect(lx - half, shaft_top, lx + half + 1, shaft_bottom);
        shape.arrow_head(lx, shaft_top - 6, 5 + half, 9);
        shape.text_centered(
            &font,
            &format!("{:03}", island.num),
            font_px as f32 * 1.25,
            lx as f32,
            ly as f32 + font_px as f32 * 0.15,
        );

        let outline = shape.grown(((font_px / 10.0) as i64).max(2));
        outline.paint(&mut diffuse, tx0 as usize, ty0 as usize, w, [0, 0, 0]);
        shape.paint(&mut diffuse, tx0 as usize, ty0 as usize, w, [255, 255, 255]);
    }

    Ok(Calibration {
        width: w as u32,
        height: h as u32,
        diffuse,
        spec: template.spec_defaults().rgb,
        islands: meta,
    })
}

impl Calibration {
    /// Write the calibration paint pair, `preview_uv.png` and `islands.json` into `dir`.
    pub fn write(&self, dir: &Path, id: &str, team: bool) -> Result<CalibrationSummary> {
        if !valid_paint_id(id) {
            return Err(Error::Invalid(
                "customer or team id must be digits only".into(),
            ));
        }
        let io = |p: &Path| {
            let name = p.display().to_string();
            move |e| Error::Io(name, e)
        };
        std::fs::create_dir_all(dir).map_err(io(dir))?;
        let (diffuse_name, spec_name) = paint_file_names(id, team);
        for (name, rgb) in [(&diffuse_name, &self.diffuse), (&spec_name, &self.spec)] {
            let path = dir.join(name);
            std::fs::write(&path, tga::encode_rgb_rle(self.width, self.height, rgb))
                .map_err(io(&path))?;
        }
        let preview = dir.join("preview_uv.png");
        image::RgbImage::from_raw(self.width, self.height, self.diffuse.clone())
            .expect("canvas sized")
            .save(&preview)
            .map_err(|e| Error::Invalid(format!("{}: {e}", preview.display())))?;
        let islands = dir.join("islands.json");
        let json = serde_json::to_string_pretty(&self.islands).expect("serialisable") + "\n";
        std::fs::write(&islands, json).map_err(io(&islands))?;
        Ok(CalibrationSummary {
            canvas: [self.width, self.height],
            island_count: self.islands.len(),
            paintable_px: self.islands.iter().map(|i| i.area_px).sum(),
            files: vec![
                diffuse_name,
                spec_name,
                "preview_uv.png".into(),
                "islands.json".into(),
            ],
        })
    }
}
