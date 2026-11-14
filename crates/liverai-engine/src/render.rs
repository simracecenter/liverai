//! Render a livery to the iRacing paint pair (diffuse + spec map) and lint it.
//!
//! A port of the proof-of-concept `render_livery.py`. The diffuse and spec map are painted
//! from the same geometry in one pass, so decal edges line up in both.

use std::path::{Path, PathBuf};

use image::{imageops, Rgb, RgbImage, RgbaImage};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::islands::{parse_panel_id, segment, Atlas, SegmentParams};
use crate::livery::{Decal, Livery};
use crate::raster::{component_boxes, BBox, Mask};
use crate::template::{Role, Template};
use crate::tga;
use crate::{Error, Result};

/// A decal pixel counts as painted above this alpha.
const DECAL_ALPHA_THRESHOLD: u8 = 16;
/// Atlas panels match segmented islands at or above this bounding-box overlap.
const PANEL_MATCH_IOU: f64 = 0.85;

pub struct RenderInput<'a> {
    pub template: &'a Template,
    pub livery: &'a Livery,
    /// Directory that decal `asset` paths are relative to.
    pub assets_dir: &'a Path,
    /// Calibrated atlas for the car, if there is one.
    pub atlas: Option<&'a Atlas>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DecalReport {
    pub id: String,
    pub panel: Option<String>,
    /// Half-open box on the canvas; may extend past it.
    pub bbox_uv: [i64; 4],
    pub keepout_number_blocks_px: usize,
    pub keepout_mandatory_px: usize,
    pub outside_paintable_px: usize,
    pub sponsor_blocks_px: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileReport {
    pub name: String,
    pub bytes: usize,
    pub sha256: String,
}

/// Loop A: deterministic checks that must pass before a paint is installed.
#[derive(Clone, Debug, Serialize)]
pub struct Lint {
    /// Both files are 24-bit RLE TGAs of the canvas size.
    pub format_ok: bool,
    /// The canvas is square and one of the sizes iRacing accepts.
    pub canvas_ok: bool,
    pub keepout_number_blocks_px: usize,
    pub keepout_mandatory_px: usize,
    pub outside_paintable_px: usize,
    /// Warning only: series contingency decals may be stamped here.
    pub sponsor_blocks_px: usize,
    pub hardware_parts_spec_preserved: bool,
    pub decals: Vec<DecalReport>,
    pub warnings: Vec<String>,
    pub all_passed: bool,
}

pub struct RenderOutput {
    pub width: u32,
    pub height: u32,
    /// RGB8, top-down.
    pub diffuse: Vec<u8>,
    /// RGB8, top-down: R metallic, G roughness, B clear coat.
    pub spec: Vec<u8>,
    /// Diffuse with the mandatory layer and number-block outlines drawn over it.
    pub preview: RgbImage,
    pub lint: Lint,
}

/// Which of the three pattern slots (red, green, blue key) covers the most paintable area,
/// in descending order. `order[0]` is the key index that takes livery slot "1".
pub fn slot_order(pattern: &RgbaImage, paintable: &Mask) -> [usize; 3] {
    let mut areas = [0usize; 3];
    for (i, p) in pattern.pixels().enumerate() {
        if !paintable.data[i] {
            continue;
        }
        let [r, g, b] = [
            p.0[0] as f64 / 255.0,
            p.0[1] as f64 / 255.0,
            p.0[2] as f64 / 255.0,
        ];
        if r > 0.78 && g < 0.25 && b < 0.25 {
            areas[0] += 1;
        } else if g > 0.78 && r < 0.25 && b < 0.25 {
            areas[1] += 1;
        } else if b > 0.78 && r < 0.25 && g < 0.25 {
            areas[2] += 1;
        }
    }
    // Ascending stable sort, reversed: the proof-of-concept's tie order.
    let mut order = [0usize, 1, 2];
    order.sort_by_key(|i| areas[*i]);
    order.reverse();
    order
}

/// Blend three values by a pattern's red/green/blue key weights. `per_key[k]` is the value
/// for key `k` (0 = red). Returns `w * h * 3` floats.
pub fn blend_by_pattern(pattern: &RgbaImage, per_key: [[f64; 3]; 3]) -> Vec<f64> {
    let mut out = Vec::with_capacity(pattern.width() as usize * pattern.height() as usize * 3);
    for p in pattern.pixels() {
        let [r, g, b] = [
            p.0[0] as f64 / 255.0,
            p.0[1] as f64 / 255.0,
            p.0[2] as f64 / 255.0,
        ];
        let s = (r + g + b).max(1e-6);
        let weights = [r / s, g / s, b / s];
        for c in 0..3 {
            let mut v = 0.0;
            for k in 0..3 {
                v += weights[k] * per_key[k][c];
            }
            out.push(v);
        }
    }
    out
}

fn to_u8(v: f64) -> u8 {
    v.clamp(0.0, 255.0) as u8
}

/// Scale an upright decal to `width_px`, then turn it by the panel's `up_angle_deg`,
/// exactly as the proof-of-concept renderer did.
fn orient_decal(asset: &RgbaImage, width_px: f64, up_angle_deg: i32, flip: bool) -> RgbaImage {
    let src = if flip {
        imageops::flip_horizontal(asset)
    } else {
        asset.clone()
    };
    let scale = width_px / src.width() as f64;
    let nw = ((src.width() as f64 * scale).round() as u32).max(1);
    let nh = ((src.height() as f64 * scale).round() as u32).max(1);
    let scaled = imageops::resize(&src, nw, nh, imageops::FilterType::Lanczos3);
    match up_angle_deg {
        180 => imageops::rotate180(&scaled),
        // A quarter turn counter-clockwise on screen (Pillow's `rotate(90)`).
        90 => imageops::rotate270(&scaled),
        // A quarter turn clockwise on screen (Pillow's `rotate(270)`).
        270 => imageops::rotate90(&scaled),
        _ => scaled,
    }
}

struct Placement {
    center: [f64; 2],
    width_px: f64,
    up_angle_deg: i32,
}

fn place(decal: &Decal, atlas: Option<&Atlas>) -> Result<Placement> {
    let panel = decal
        .panel
        .as_deref()
        .and_then(|id| atlas.and_then(|a| a.panel(id)));
    let need_panel = || {
        Error::Invalid(format!(
            "decal '{}' is placed relative to a panel, but panel {:?} is not in the car's atlas",
            decal.id, decal.panel
        ))
    };
    let up_angle_deg = match decal.up_angle_deg {
        Some(a) => a,
        None => panel.and_then(|p| p.up_angle_deg).unwrap_or(0),
    };
    let center = match (decal.center_uv, decal.center_panel) {
        (Some(c), _) => c,
        (None, Some([u, v])) => {
            let b = panel.ok_or_else(need_panel)?.bbox;
            [
                b[0] as f64 + u * (b[2] - b[0]) as f64,
                b[1] as f64 + v * (b[3] - b[1]) as f64,
            ]
        }
        (None, None) => {
            return Err(Error::Invalid(format!(
                "decal '{}' has no centre",
                decal.id
            )))
        }
    };
    let width_px = match (decal.width_px, decal.width_panel) {
        (Some(w), _) => w,
        (None, Some(f)) => {
            let b = panel.ok_or_else(need_panel)?.bbox;
            // The decal's baseline runs along x when its top points along y, and vice versa.
            let extent = if up_angle_deg == 0 || up_angle_deg == 180 {
                b[2] - b[0]
            } else {
                b[3] - b[1]
            };
            f * extent as f64
        }
        (None, None) => return Err(Error::Invalid(format!("decal '{}' has no width", decal.id))),
    };
    if !(width_px.is_finite() && width_px >= 1.0) {
        return Err(Error::Invalid(format!(
            "decal '{}' has a width below 1 px",
            decal.id
        )));
    }
    Ok(Placement {
        center,
        width_px,
        up_angle_deg,
    })
}

pub fn render(input: &RenderInput) -> Result<RenderOutput> {
    let (template, livery) = (input.template, input.livery);
    livery.validate()?;
    let (w, h) = (template.width() as usize, template.height() as usize);
    let n = w * h;

    let paintable = template.paintable()?;
    let mandatory_rgba = template.role_rgba(Role::Mandatory).ok();
    let mandatory = template.role_mask(Role::Mandatory, 0);
    let number_blocks = template.role_mask(Role::NumberBlocks, 0);
    let sponsor_blocks = template.role_mask(Role::SponsorBlocks, 0);
    let mut warnings = Vec::new();

    // 1. Base pattern: recolour the three slots and paint each slot's finish.
    let pattern = template.pattern(livery.base.pattern.id()?)?;
    let order = slot_order(&pattern, &paintable);
    let mut key_colour = [[0.0f64; 3]; 3];
    let mut key_finish = [[0.0f64; 3]; 3];
    for (rank, key) in order.iter().enumerate() {
        let slot = &livery.base.slots[&(rank + 1).to_string()];
        key_colour[*key] = livery.colour(slot.colour())?.map(f64::from);
        key_finish[*key] = livery.finish(slot.finish())?.triple();
    }
    let mut diffuse: Vec<u8> = blend_by_pattern(&pattern, key_colour)
        .into_iter()
        .map(to_u8)
        .collect();
    let mut spec: Vec<u8> = blend_by_pattern(&pattern, key_finish)
        .into_iter()
        .map(to_u8)
        .collect();

    // 2. Per-panel colour and finish overrides.
    if !livery.panel_finishes.is_empty() {
        let wire = template.role_rgba(Role::Wire)?;
        let islands = segment(&paintable, &wire, &SegmentParams::default());
        let matches = input
            .atlas
            .map(|a| a.match_islands(&islands.meta(), PANEL_MATCH_IOU));
        for pf in &livery.panel_finishes {
            let island = match &matches {
                Some(m) => m
                    .iter()
                    .find(|pm| pm.panel == pf.panel)
                    .and_then(|pm| pm.island),
                None => parse_panel_id(&pf.panel).filter(|num| *num <= islands.count),
            };
            let Some(num) = island else {
                return Err(Error::Invalid(format!(
                    "panel '{}' does not match an island of this template",
                    pf.panel
                )));
            };
            let colour = pf.colour.as_deref().map(|c| livery.colour(c)).transpose()?;
            let finish = pf
                .finish
                .as_deref()
                .map(|f| livery.finish(Some(f)))
                .transpose()?;
            for i in 0..n {
                if islands.labels.data[i] == num && paintable.data[i] && !mandatory.data[i] {
                    if let Some(c) = colour {
                        diffuse[i * 3..i * 3 + 3].copy_from_slice(&c);
                    }
                    if let Some(f) = finish {
                        diffuse_spec(&mut spec, i, [f.metallic, f.roughness, f.clearcoat]);
                    }
                }
            }
        }
        if input.atlas.is_none() {
            warnings.push(
                "panel_finishes resolved by island rank because no atlas was supplied".into(),
            );
        }
    }

    // 3. Decals, composited into the diffuse and the spec map with the same alpha.
    let mut decal_union = Mask::new(w, h, false);
    let mut decal_reports = Vec::new();
    for decal in &livery.decals {
        let placement = place(decal, input.atlas)?;
        let asset_path: PathBuf = input.assets_dir.join(&decal.asset);
        let asset = image::open(&asset_path)
            .map_err(|e| {
                Error::Invalid(format!(
                    "decal '{}': cannot read {}: {e}",
                    decal.id,
                    asset_path.display()
                ))
            })?
            .to_rgba8();
        let placed = orient_decal(
            &asset,
            placement.width_px,
            placement.up_angle_deg,
            decal.flip_horizontal,
        );
        let x0 = (placement.center[0] - placed.width() as f64 / 2.0).round() as i64;
        let y0 = (placement.center[1] - placed.height() as f64 / 2.0).round() as i64;
        let finish = livery.finish(decal.finish.as_deref())?.triple();

        let mut report = DecalReport {
            id: decal.id.clone(),
            panel: decal.panel.clone(),
            bbox_uv: [
                x0,
                y0,
                x0 + placed.width() as i64,
                y0 + placed.height() as i64,
            ],
            keepout_number_blocks_px: 0,
            keepout_mandatory_px: 0,
            outside_paintable_px: 0,
            sponsor_blocks_px: 0,
        };
        for (dx, dy, px) in placed.enumerate_pixels() {
            let (x, y) = (x0 + dx as i64, y0 + dy as i64);
            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                continue;
            }
            let [r, g, b, a] = px.0;
            if a == 0 {
                continue;
            }
            let i = y as usize * w + x as usize;
            let af = a as f64 / 255.0;
            for (c, src) in [r, g, b].into_iter().enumerate() {
                let dst = diffuse[i * 3 + c] as f64;
                diffuse[i * 3 + c] = (src as f64 * af + dst * (1.0 - af))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let s = spec[i * 3 + c] as f64;
                spec[i * 3 + c] = to_u8(s * (1.0 - af) + finish[c] * af);
            }
            if a > DECAL_ALPHA_THRESHOLD {
                decal_union.data[i] = true;
                report.keepout_number_blocks_px += number_blocks.data[i] as usize;
                report.keepout_mandatory_px += mandatory.data[i] as usize;
                report.outside_paintable_px += !paintable.data[i] as usize;
                report.sponsor_blocks_px += sponsor_blocks.data[i] as usize;
            }
        }
        decal_reports.push(report);
    }

    // 4. Outside the paintable mask nothing shows on the car: paint it flat. Hardware keeps
    //    the template's material values.
    let slot1 = &livery.base.slots["1"];
    let fill = livery.colour(
        livery
            .unpaintable_colour
            .as_deref()
            .unwrap_or(slot1.colour()),
    )?;
    for i in 0..n {
        if !paintable.data[i] {
            diffuse[i * 3..i * 3 + 3].copy_from_slice(&fill);
        }
    }
    let defaults = template.spec_defaults();
    if !defaults.from_template {
        warnings.push(
            "template has no Custom Spec Map; hardware spec values cannot be preserved".into(),
        );
    }
    for i in 0..n {
        if defaults.parts.data[i] {
            spec[i * 3..i * 3 + 3].copy_from_slice(&defaults.rgb[i * 3..i * 3 + 3]);
        }
    }
    let hardware_ok = (0..n)
        .filter(|i| defaults.parts.data[*i])
        .all(|i| spec[i * 3..i * 3 + 3] == defaults.rgb[i * 3..i * 3 + 3]);

    // 5. Preview.
    let mut preview =
        RgbImage::from_raw(w as u32, h as u32, diffuse.clone()).expect("canvas sized");
    if let Some(m) = &mandatory_rgba {
        for (dst, src) in preview.pixels_mut().zip(m.pixels()) {
            let a = src.0[3] as u32;
            if a > 0 {
                for c in 0..3 {
                    dst.0[c] =
                        ((src.0[c] as u32 * a + dst.0[c] as u32 * (255 - a) + 127) / 255) as u8;
                }
            }
        }
    }
    for b in component_boxes(&number_blocks) {
        outline(&mut preview, b, 3, Rgb([255, 255, 255]));
    }

    // 6. Lint.
    let count = |m: &Mask| {
        (0..n)
            .filter(|i| decal_union.data[*i] && m.data[*i])
            .count()
    };
    let nb = count(&number_blocks);
    let cm = count(&mandatory);
    let sb = count(&sponsor_blocks);
    let outside = (0..n)
        .filter(|i| decal_union.data[*i] && !paintable.data[*i])
        .count();
    if sb > 0 {
        warnings.push(format!(
            "{sb} px of decals overlap Sponsor Blocks; check the series' contingency rules"
        ));
    }
    let diffuse_tga = tga::encode_rgb_rle(w as u32, h as u32, &diffuse);
    let spec_tga = tga::encode_rgb_rle(w as u32, h as u32, &spec);
    let header_ok = |bytes: &[u8]| {
        tga::read_header(bytes).is_some_and(|hd| {
            hd.image_type == tga::IMAGE_TYPE_RLE_TRUECOLOR
                && hd.bits_per_pixel == 24
                && hd.width as usize == w
                && hd.height as usize == h
        })
    };
    let format_ok = header_ok(&diffuse_tga) && header_ok(&spec_tga);
    let canvas_ok = w == h && (w == 2048 || w == 1024);
    let all_passed = format_ok && canvas_ok && nb == 0 && cm == 0 && outside == 0 && hardware_ok;

    Ok(RenderOutput {
        width: w as u32,
        height: h as u32,
        diffuse,
        spec,
        preview,
        lint: Lint {
            format_ok,
            canvas_ok,
            keepout_number_blocks_px: nb,
            keepout_mandatory_px: cm,
            outside_paintable_px: outside,
            sponsor_blocks_px: sb,
            hardware_parts_spec_preserved: hardware_ok,
            decals: decal_reports,
            warnings,
            all_passed,
        },
    })
}

fn diffuse_spec(spec: &mut [u8], i: usize, v: [u8; 3]) {
    spec[i * 3..i * 3 + 3].copy_from_slice(&v);
}

fn outline(img: &mut RgbImage, b: BBox, width: usize, colour: Rgb<u8>) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    for y in b[1]..b[3].min(h) {
        for x in b[0]..b[2].min(w) {
            let edge =
                x < b[0] + width || x + width >= b[2] || y < b[1] + width || y + width >= b[3];
            if edge {
                img.put_pixel(x as u32, y as u32, colour);
            }
        }
    }
}

/// Names of the paint files for a customer or team id.
pub fn paint_file_names(id: &str, team: bool) -> (String, String) {
    if team {
        (
            format!("car_team_{id}.tga"),
            format!("car_spec_team_{id}.tga"),
        )
    } else {
        (format!("car_{id}.tga"), format!("car_spec_{id}.tga"))
    }
}

/// An iRacing customer or team id: digits only, so it is safe in a file name.
pub fn valid_paint_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 12 && id.bytes().all(|b| b.is_ascii_digit())
}

#[derive(Clone, Debug, Serialize)]
pub struct WrittenRender {
    pub files: Vec<FileReport>,
    pub lint: Lint,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl RenderOutput {
    /// Write the paint pair, `preview_uv.png` and `lint.json` into `dir`.
    pub fn write(&self, dir: &Path, id: &str, team: bool) -> Result<WrittenRender> {
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
        let mut files = Vec::new();
        for (name, rgb) in [(diffuse_name, &self.diffuse), (spec_name, &self.spec)] {
            let bytes = tga::encode_rgb_rle(self.width, self.height, rgb);
            let path = dir.join(&name);
            std::fs::write(&path, &bytes).map_err(io(&path))?;
            files.push(FileReport {
                name,
                bytes: bytes.len(),
                sha256: sha256_hex(&bytes),
            });
        }
        let preview_path = dir.join("preview_uv.png");
        self.preview
            .save(&preview_path)
            .map_err(|e| Error::Invalid(format!("{}: {e}", preview_path.display())))?;
        let written = WrittenRender {
            files,
            lint: self.lint.clone(),
        };
        let lint_path = dir.join("lint.json");
        let json = serde_json::to_string_pretty(&written).expect("serialisable") + "\n";
        std::fs::write(&lint_path, json).map_err(io(&lint_path))?;
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_ids_are_digits_only() {
        assert!(valid_paint_id("341237"));
        assert!(!valid_paint_id(""));
        assert!(!valid_paint_id("12/../x"));
        assert!(!valid_paint_id("1234567890123"));
        assert_eq!(paint_file_names("7", false).0, "car_7.tga");
        assert_eq!(paint_file_names("7", true).1, "car_spec_team_7.tga");
    }

    #[test]
    fn decal_orientation_follows_up_angle() {
        // 2x1 image: left pixel red, right pixel blue. "Up" is the top edge.
        let mut src = RgbaImage::new(2, 1);
        src.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        src.put_pixel(1, 0, image::Rgba([0, 0, 255, 255]));
        let red = |im: &RgbaImage, x, y| im.get_pixel(x, y).0[0] == 255;

        let up0 = orient_decal(&src, 2.0, 0, false);
        assert!(red(&up0, 0, 0));
        let up180 = orient_decal(&src, 2.0, 180, false);
        assert!(red(&up180, 1, 0));
        // Quarter turn counter-clockwise: the decal's left edge (red) ends at the bottom.
        let up90 = orient_decal(&src, 2.0, 90, false);
        assert_eq!((up90.width(), up90.height()), (1, 2));
        assert!(red(&up90, 0, 1));
        // Quarter turn clockwise: the left edge ends at the top.
        let up270 = orient_decal(&src, 2.0, 270, false);
        assert!(red(&up270, 0, 0));
        let flipped = orient_decal(&src, 2.0, 0, true);
        assert!(red(&flipped, 1, 0));
    }
}
