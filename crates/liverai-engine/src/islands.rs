//! UV island segmentation and the panel atlas.
//!
//! `segment` is a port of the proof-of-concept `segment_islands`: islands are the paintable
//! regions of the template, split further along the green panel outlines of the `Wire` layer.
//! Island ids are ranked by area (`p001` is the largest), which is how the calibrated
//! `panels.json` atlases were numbered.

use std::path::Path;

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::raster::{
    close_square, edt, label4, label_areas, label_bboxes, max3x3, min3x3, BBox, Grid, Mask,
};
use crate::{Error, Result};

/// Thresholds tuned on 2048 px templates.
#[derive(Clone, Copy, Debug)]
pub struct SegmentParams {
    /// Green outline: `g - max(r, b)` above this, with alpha above `wire_alpha`.
    pub green_margin: i32,
    pub wire_alpha: u8,
    /// Half-size of the square used to close gaps in the outlines (3 = 7x7).
    pub close_radius: usize,
    /// Minimum area of an outlined region, and its minimum paintable area, to seed an island.
    pub min_region_px: usize,
    pub min_region_paintable_px: usize,
    /// Leftover paintable area seeds an island where it is at least this far from any outline...
    pub leftover_core_dist: f64,
    /// ...over at least this many pixels.
    pub min_leftover_core_px: usize,
    /// Rounds of growing seeds across unlabelled paintable pixels.
    pub grow_rounds: usize,
    /// Islands smaller than this are dropped.
    pub min_island_px: usize,
}

impl Default for SegmentParams {
    fn default() -> Self {
        Self {
            green_margin: 15,
            wire_alpha: 20,
            close_radius: 3,
            min_region_px: 1200,
            min_region_paintable_px: 1000,
            leftover_core_dist: 22.0,
            min_leftover_core_px: 400,
            grow_rounds: 40,
            min_island_px: 1200,
        }
    }
}

/// Segmented islands: `labels` holds 1-based island ids ranked by area, 0 elsewhere.
pub struct Islands {
    pub labels: Grid<u32>,
    pub count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IslandMeta {
    /// `p001`, `p002`, ...
    pub id: String,
    pub num: u32,
    pub bbox: BBox,
    pub area_px: usize,
    /// Pole of inaccessibility: the interior point farthest from the island's edge.
    pub center: [usize; 2],
    pub max_inscribed_r: f64,
    pub calibration_hex: String,
}

pub fn panel_id(num: u32) -> String {
    format!("p{num:03}")
}

/// `p004` -> 4.
pub fn parse_panel_id(id: &str) -> Option<u32> {
    id.strip_prefix('p')
        .and_then(|d| d.parse().ok())
        .filter(|n| *n > 0)
}

pub fn segment(paintable: &Mask, wire: &RgbaImage, params: &SegmentParams) -> Islands {
    let (w, h) = (paintable.w, paintable.h);
    let green = Mask {
        w,
        h,
        data: wire
            .pixels()
            .map(|p| {
                let [r, g, b, a] = p.0;
                g as i32 - r.max(b) as i32 > params.green_margin && a > params.wire_alpha
            })
            .collect(),
    };
    let open = close_square(&green, params.close_radius).not();

    // 1. Regions enclosed by green outlines seed islands.
    let (regions, n_regions) = label4(&open);
    let corners = [0, w - 1, (h - 1) * w, h * w - 1].map(|i| regions.data[i]);
    let region_areas = label_areas(&regions, n_regions);
    let mut region_paintable = vec![0usize; n_regions as usize];
    for i in 0..w * h {
        let l = regions.data[i];
        if l > 0 && paintable.data[i] {
            region_paintable[(l - 1) as usize] += 1;
        }
    }
    let mut seed_of_region = vec![0u32; n_regions as usize + 1];
    let mut next_id = 1u32;
    for l in 1..=n_regions {
        let i = (l - 1) as usize;
        if !corners.contains(&l)
            && region_areas[i] >= params.min_region_px
            && region_paintable[i] >= params.min_region_paintable_px
        {
            seed_of_region[l as usize] = next_id;
            next_id += 1;
        }
    }
    let mut seeds = regions.map(|l| seed_of_region[*l as usize]);

    // 2. Paintable area not covered by a seed (islands without a closed outline) seeds from its core.
    let leftover = Mask {
        w,
        h,
        data: (0..w * h)
            .map(|i| paintable.data[i] && open.data[i] && seeds.data[i] == 0)
            .collect(),
    };
    let (dist, _) = edt(&leftover);
    let core = dist.map(|d| *d >= params.leftover_core_dist);
    let (cores, n_cores) = label4(&core);
    let core_areas = label_areas(&cores, n_cores);
    let mut seed_of_core = vec![0u32; n_cores as usize + 1];
    for l in 1..=n_cores {
        if core_areas[(l - 1) as usize] >= params.min_leftover_core_px {
            seed_of_core[l as usize] = next_id;
            next_id += 1;
        }
    }
    for i in 0..w * h {
        let s = seed_of_core[cores.data[i] as usize];
        if s > 0 {
            seeds.data[i] = s;
        }
    }

    // 3. Grow seeds across paintable pixels that are not on an outline.
    let mut grown = seeds;
    for _ in 0..params.grow_rounds {
        let dilated = max3x3(&grown);
        let mut changed = false;
        for i in 0..w * h {
            if grown.data[i] == 0 && paintable.data[i] && open.data[i] && dilated.data[i] > 0 {
                grown.data[i] = dilated.data[i];
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // 4. Every remaining paintable pixel takes the nearest island's id.
    let unlabelled = grown.map(|l| *l != 0);
    // `edt` measures distance to the nearest *unset* pixel, so invert: unset = labelled.
    let (_, nearest) = edt(&unlabelled.not());
    let n_raw = next_id - 1;
    let mut raw = Grid::new(w, h, 0u32);
    if n_raw > 0 {
        for i in 0..w * h {
            if paintable.data[i] {
                raw.data[i] = grown.data[nearest.data[i]];
            }
        }
    }

    // 5. Rank by area, largest first; drop slivers. Ties keep the proof-of-concept order
    //    (ascending stable sort, reversed).
    let areas = label_areas(&raw, n_raw);
    let mut order: Vec<usize> = (0..n_raw as usize).collect();
    order.sort_by_key(|i| areas[*i]);
    order.reverse();
    let mut remap = vec![0u32; n_raw as usize + 1];
    let mut count = 0u32;
    for old in order {
        if areas[old] >= params.min_island_px {
            count += 1;
            remap[old + 1] = count;
        }
    }
    Islands {
        labels: raw.map(|l| remap[*l as usize]),
        count,
    }
}

/// Golden-ratio hue spacing with alternating saturation and value, so neighbours contrast.
pub fn calibration_colour(num: u32) -> [u8; 3] {
    let hue = ((num - 1) as f64 * 0.618_033_988_749_895) % 1.0;
    let s = if num % 2 == 1 { 0.80 } else { 0.60 };
    let v = if num % 3 != 0 { 0.92 } else { 0.75 };
    hsv_to_rgb(hue, s, v)
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [u8; 3] {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match (i as i64).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b].map(|c| (c * 255.0).round() as u8)
}

impl Islands {
    /// Pixels on a boundary between two islands, or between an island and the background.
    pub fn edges(&self) -> Mask {
        let hi = max3x3(&self.labels);
        let lo = min3x3(&self.labels);
        Mask {
            w: self.labels.w,
            h: self.labels.h,
            data: hi.data.iter().zip(&lo.data).map(|(a, b)| a != b).collect(),
        }
    }

    pub fn mask(&self, num: u32) -> Mask {
        self.labels.map(|l| *l == num)
    }

    /// Geometry of every island, in id order.
    pub fn meta(&self) -> Vec<IslandMeta> {
        let (w, h) = (self.labels.w, self.labels.h);
        let boxes = label_bboxes(&self.labels, self.count);
        let areas = label_areas(&self.labels, self.count);
        let edges = self.edges();
        (1..=self.count)
            .map(|num| {
                let bbox = boxes[(num - 1) as usize].unwrap_or([0, 0, 0, 0]);
                // Work on the island's bounding box (padded by one pixel) to keep this cheap.
                let (x0, y0) = (bbox[0].saturating_sub(1), bbox[1].saturating_sub(1));
                let (x1, y1) = ((bbox[2] + 1).min(w), (bbox[3] + 1).min(h));
                let (bw, bh) = (x1 - x0, y1 - y0);
                let mut inner = Mask::new(bw, bh, false);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = y * w + x;
                        inner.data[(y - y0) * bw + (x - x0)] =
                            self.labels.data[i] == num && !edges.data[i];
                    }
                }
                let (dist, _) = edt(&inner);
                let (mut best, mut best_i) = (0.0f64, 0usize);
                for (i, d) in dist.data.iter().enumerate() {
                    if inner.data[i] && d.is_finite() && *d > best {
                        best = *d;
                        best_i = i;
                    }
                }
                let [r, g, b] = calibration_colour(num);
                IslandMeta {
                    id: panel_id(num),
                    num,
                    bbox,
                    area_px: areas[(num - 1) as usize],
                    center: [x0 + best_i % bw.max(1), y0 + best_i / bw.max(1)],
                    max_inscribed_r: (best * 10.0).round() / 10.0,
                    calibration_hex: format!("#{r:02X}{g:02X}{b:02X}"),
                }
            })
            .collect()
    }
}

/// One labelled panel of a calibrated atlas (`panels.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Panel {
    pub id: String,
    pub bbox: BBox,
    pub area_px: usize,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub up_angle_deg: Option<i32>,
    #[serde(default)]
    pub forward: Option<String>,
    #[serde(default)]
    pub mirror_of: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>,
}

/// A car's calibrated panel atlas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Atlas {
    pub car: String,
    pub canvas: [u32; 2],
    #[serde(default)]
    pub confirmed_coverage_ratio: Option<f64>,
    pub panels: Vec<Panel>,
}

/// How a segmented island lines up with an atlas panel.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PanelMatch {
    pub panel: String,
    /// Island number in the current segmentation, if one overlaps the panel well enough.
    pub island: Option<u32>,
    pub iou: f64,
}

fn iou(a: BBox, b: BBox) -> f64 {
    let ix = a[2].min(b[2]).saturating_sub(a[0].max(b[0]));
    let iy = a[3].min(b[3]).saturating_sub(a[1].max(b[1]));
    let inter = (ix * iy) as f64;
    let area = |r: BBox| ((r[2] - r[0]) * (r[3] - r[1])) as f64;
    let union = area(a) + area(b) - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

impl Atlas {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text =
            std::fs::read_to_string(path).map_err(|e| Error::Io(path.display().to_string(), e))?;
        serde_json::from_str(&text).map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))
    }

    pub fn panel(&self, id: &str) -> Option<&Panel> {
        self.panels.iter().find(|p| p.id == id)
    }

    /// Match atlas panels to freshly segmented islands by bounding box, not by rank.
    ///
    /// Island ids are area ranks, so two islands of nearly equal area can swap ids between
    /// segmentation implementations. Matching on geometry keeps the atlas labels attached to
    /// the right pixels; a panel with no island at `min_iou` or better is reported unmatched.
    pub fn match_islands(&self, islands: &[IslandMeta], min_iou: f64) -> Vec<PanelMatch> {
        let mut taken = vec![false; islands.len()];
        self.panels
            .iter()
            .map(|panel| {
                let best = islands
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !taken[*i])
                    .map(|(i, isl)| (i, iou(panel.bbox, isl.bbox)))
                    .max_by(|a, b| a.1.total_cmp(&b.1));
                match best {
                    Some((i, score)) if score >= min_iou => {
                        taken[i] = true;
                        PanelMatch {
                            panel: panel.id.clone(),
                            island: Some(islands[i].num),
                            iou: score,
                        }
                    }
                    other => PanelMatch {
                        panel: panel.id.clone(),
                        island: None,
                        iou: other.map(|b| b.1).unwrap_or(0.0),
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_ids_round_trip() {
        assert_eq!(panel_id(4), "p004");
        assert_eq!(parse_panel_id("p004"), Some(4));
        assert_eq!(parse_panel_id("p000"), None);
        assert_eq!(parse_panel_id("door"), None);
    }

    #[test]
    fn calibration_colours_match_the_atlas() {
        // Values recorded in cars/porsche992rgt3/atlas/panels.json.
        assert_eq!(calibration_colour(1), [0xEB, 0x2F, 0x2F]);
        assert_eq!(calibration_colour(2), [0x5E, 0x87, 0xEB]);
        assert_eq!(calibration_colour(3), [0x80, 0xBF, 0x26]);
        assert_eq!(calibration_colour(4), [0xEB, 0x5E, 0xD9]);
        assert_eq!(calibration_colour(5), [0x2F, 0xEB, 0xCB]);
    }

    #[test]
    fn atlas_matches_by_geometry_not_rank() {
        let atlas = Atlas {
            car: "test".into(),
            canvas: [100, 100],
            confirmed_coverage_ratio: None,
            panels: vec![
                Panel {
                    id: "p001".into(),
                    bbox: [0, 0, 50, 50],
                    area_px: 2500,
                    label: None,
                    up_angle_deg: None,
                    forward: None,
                    mirror_of: None,
                    confidence: None,
                },
                Panel {
                    id: "p002".into(),
                    bbox: [50, 50, 100, 99],
                    area_px: 2450,
                    label: None,
                    up_angle_deg: None,
                    forward: None,
                    mirror_of: None,
                    confidence: None,
                },
                Panel {
                    id: "p003".into(),
                    bbox: [0, 80, 10, 90],
                    area_px: 100,
                    label: None,
                    up_angle_deg: None,
                    forward: None,
                    mirror_of: None,
                    confidence: None,
                },
            ],
        };
        let isl = |num: u32, bbox: BBox| IslandMeta {
            id: panel_id(num),
            num,
            bbox,
            area_px: 0,
            center: [0, 0],
            max_inscribed_r: 0.0,
            calibration_hex: String::new(),
        };
        // The two big islands swapped rank.
        let islands = vec![isl(1, [50, 50, 100, 100]), isl(2, [0, 0, 50, 50])];
        let m = atlas.match_islands(&islands, 0.9);
        assert_eq!(m[0].island, Some(2));
        assert_eq!(m[1].island, Some(1));
        assert_eq!(m[2].island, None);
    }
}
