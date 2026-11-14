//! Pre-render intrusion scan.
//!
//! Before a decal is placed, its bounding box is tested against the template's keep-outs.
//! A text or logo box must sit on uninterrupted sheet metal: any unpaintable pixel inside it
//! means a door handle, louvre, duct or panel gap would cut through the artwork.

use serde::{Deserialize, Serialize};

use crate::raster::Mask;
use crate::template::{Role, Template};
use crate::Result;

/// `Car Decal` pixels count above this alpha.
const DECAL_ALPHA: u8 = 20;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanBox {
    pub name: String,
    /// Half-open box `[x0, y0, x1, y1)` on the canvas.
    pub bbox: [i64; 4],
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ScanResult {
    pub name: String,
    pub bbox: [i64; 4],
    pub area_px: usize,
    pub number_blocks_px: usize,
    pub mandatory_px: usize,
    pub unpaintable_px: usize,
    pub car_decal_px: usize,
    /// No number block or mandatory pixel inside: acceptable for a full-bleed panel fill.
    pub clear_for_fill: bool,
    /// Nothing at all inside: required for text and logo lockups.
    pub clear_for_text: bool,
}

/// The template masks a scan needs, read once.
pub struct Keepouts {
    pub number_blocks: Mask,
    pub mandatory: Mask,
    pub unpaintable: Mask,
    pub car_decal: Mask,
}

impl Keepouts {
    pub fn from_template(template: &Template) -> Result<Self> {
        Ok(Self {
            number_blocks: template.role_mask(Role::NumberBlocks, 0),
            mandatory: template.role_mask(Role::Mandatory, 0),
            unpaintable: template.paintable()?.not(),
            car_decal: template.role_mask(Role::Decal, DECAL_ALPHA),
        })
    }

    pub fn scan(&self, boxes: &[ScanBox]) -> Vec<ScanResult> {
        boxes
            .iter()
            .map(|b| {
                let [x0, y0, x1, y1] = b.bbox;
                let (w, h) = (self.unpaintable.w as i64, self.unpaintable.h as i64);
                let inside_w = (x1.clamp(0, w) - x0.clamp(0, w)).max(0);
                let inside_h = (y1.clamp(0, h) - y0.clamp(0, h)).max(0);
                let area_px = ((x1 - x0).max(0) * (y1 - y0).max(0)) as usize;
                // Anything hanging off the canvas is unpaintable too.
                let off_canvas = area_px - (inside_w * inside_h) as usize;
                let nb = self.number_blocks.count_in_box(x0, y0, x1, y1);
                let cm = self.mandatory.count_in_box(x0, y0, x1, y1);
                let unp = self.unpaintable.count_in_box(x0, y0, x1, y1) + off_canvas;
                let cd = self.car_decal.count_in_box(x0, y0, x1, y1);
                ScanResult {
                    name: b.name.clone(),
                    bbox: b.bbox,
                    area_px,
                    number_blocks_px: nb,
                    mandatory_px: cm,
                    unpaintable_px: unp,
                    car_decal_px: cd,
                    clear_for_fill: area_px > 0 && nb == 0 && cm == 0,
                    clear_for_text: area_px > 0 && nb == 0 && cm == 0 && unp == 0 && cd == 0,
                }
            })
            .collect()
    }
}
