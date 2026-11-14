//! Reading an iRacing paint template (`.psd`).
//!
//! A template is a UV atlas with helper layers. Layer names vary between cars
//! (`Car_Mandatory`, `Car Mandatory`, `Car mandatory`, ...), so lookups normalise names
//! instead of matching them exactly.

use std::collections::BTreeMap;
use std::path::Path;

use image::RgbaImage;
use serde::Serialize;

use crate::raster::{component_boxes, BBox, Mask};
use crate::{Error, Result};

/// Spec map values used when a template has no `Custom Spec Map` group:
/// dielectric, glossy, no clear coat (the 992 GT3 R template's base paint).
pub const FALLBACK_SPEC: [u8; 3] = [0, 51, 0];

#[derive(Clone, Debug, Serialize)]
pub struct LayerInfo {
    pub name: String,
    /// Group names from the root down to the layer's parent.
    pub path: Vec<String>,
    pub visible: bool,
    /// Half-open pixel box `[x0, y0, x1, y1)`.
    pub bbox: [i32; 4],
    #[serde(skip)]
    index: usize,
}

/// The helper layers the engine uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Mask,
    Wire,
    NumberBlocks,
    Mandatory,
    SponsorBlocks,
    Decal,
}

impl Role {
    fn names(self) -> &'static [&'static str] {
        match self {
            Role::Mask => &["mask"],
            Role::Wire => &["wire"],
            Role::NumberBlocks => &[
                "number blocks",
                "number boards",
                "number block",
                "number board",
            ],
            Role::Mandatory => &["car mandatory"],
            Role::SponsorBlocks => &["sponsor blocks", "sponsor block"],
            Role::Decal => &["car decal"],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Mask => "Mask",
            Role::Wire => "Wire",
            Role::NumberBlocks => "Number Blocks",
            Role::Mandatory => "Car Mandatory",
            Role::SponsorBlocks => "Sponsor Blocks",
            Role::Decal => "Car Decal",
        }
    }
}

/// Lower-case, underscores to spaces, collapsed whitespace.
pub fn normalise(name: &str) -> String {
    name.replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `car_pattern_012.tga` -> `12`.
fn pattern_id(name: &str) -> Option<u32> {
    let n = normalise(name);
    let rest = n.strip_prefix("car pattern ")?;
    let digits = rest.strip_suffix(".tga").unwrap_or(rest);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Default material values and the hardware pixels that must keep them.
pub struct SpecDefaults {
    /// Template spec map, RGB8 top-down (R metallic, G roughness, B clear coat).
    pub rgb: Vec<u8>,
    /// Pixels covered by a `Parts` layer in any channel group.
    pub parts: Mask,
    /// False when the template has no `Custom Spec Map` and [`FALLBACK_SPEC`] was used.
    pub from_template: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Inspection {
    pub canvas: [u32; 2],
    pub layers: Vec<LayerInfo>,
    pub pattern_ids: Vec<u32>,
    pub paintable_px: usize,
    pub paintable_ratio: f64,
    pub number_block_boxes: Vec<BBox>,
    pub mandatory_boxes: Vec<BBox>,
    pub sponsor_block_boxes: Vec<BBox>,
    pub decal_boxes: Vec<BBox>,
    pub has_spec_map: bool,
    pub hardware_parts_px: usize,
    /// Roles the template does not provide. `Mask` and `Wire` are required for rendering.
    pub missing: Vec<String>,
}

pub struct Template {
    psd: psd::Psd,
    layers: Vec<LayerInfo>,
    /// Group id -> (name, parent id).
    groups: BTreeMap<u32, (String, Option<u32>)>,
}

impl Template {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| Error::Io(path.display().to_string(), e))?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let psd = psd::Psd::from_bytes(bytes)
            .map_err(|e| Error::Template(format!("not a readable PSD: {e}")))?;
        if psd.depth() != psd::PsdDepth::Eight {
            return Err(Error::Template("only 8-bit templates are supported".into()));
        }
        if psd.color_mode() != psd::ColorMode::Rgb {
            return Err(Error::Template("only RGB templates are supported".into()));
        }
        let groups: BTreeMap<u32, (String, Option<u32>)> = psd
            .groups()
            .iter()
            .map(|(id, g)| (*id, (g.name().to_string(), g.parent_id())))
            .collect();
        let group_path = |mut id: Option<u32>| {
            let mut path = Vec::new();
            while let Some(g) = id.and_then(|i| groups.get(&i)) {
                path.push(g.0.clone());
                id = g.1;
            }
            path.reverse();
            path
        };
        let layers = psd
            .layers()
            .iter()
            .enumerate()
            .map(|(index, l)| LayerInfo {
                name: l.name().to_string(),
                path: group_path(l.parent_id()),
                // psd 0.3 reports the "hidden" flag inverted.
                visible: !l.visible(),
                bbox: [
                    l.layer_left(),
                    l.layer_top(),
                    l.layer_right() + 1,
                    l.layer_bottom() + 1,
                ],
                index,
            })
            .collect();
        Ok(Self {
            psd,
            layers,
            groups,
        })
    }

    pub fn width(&self) -> u32 {
        self.psd.width()
    }

    pub fn height(&self) -> u32 {
        self.psd.height()
    }

    pub fn layers(&self) -> &[LayerInfo] {
        &self.layers
    }

    fn rgba_of(&self, layer: &LayerInfo) -> RgbaImage {
        let data = self.psd.layers()[layer.index].rgba();
        RgbaImage::from_raw(self.width(), self.height(), data).expect("psd layer is canvas sized")
    }

    fn find_role(&self, role: Role) -> Option<&LayerInfo> {
        let names = role.names();
        let matches: Vec<&LayerInfo> = self
            .layers
            .iter()
            .filter(|l| names.contains(&normalise(&l.name).as_str()))
            .collect();
        // Prefer the helper group ("Turn Off Before Exporting TGA") over look-alikes elsewhere,
        // e.g. a hidden "IMSA Car_decal" variant is not matched, but a "Mask" in another group could be.
        matches
            .iter()
            .find(|l| l.path.iter().any(|g| normalise(g).contains("turn off")))
            .or(matches.first())
            .copied()
    }

    /// Full-canvas RGBA of a helper layer.
    pub fn role_rgba(&self, role: Role) -> Result<RgbaImage> {
        self.find_role(role)
            .map(|l| self.rgba_of(l))
            .ok_or_else(|| Error::MissingLayer(role.label().into()))
    }

    /// Pixels where a helper layer's alpha is above `threshold`. Missing optional layers give an empty mask.
    pub fn role_mask(&self, role: Role, threshold: u8) -> Mask {
        let (w, h) = (self.width() as usize, self.height() as usize);
        match self.find_role(role) {
            Some(l) => Mask {
                w,
                h,
                data: self
                    .rgba_of(l)
                    .pixels()
                    .map(|p| p.0[3] > threshold)
                    .collect(),
            },
            None => Mask::new(w, h, false),
        }
    }

    /// Paintable pixels: where the `Mask` layer is fully transparent.
    pub fn paintable(&self) -> Result<Mask> {
        if self.find_role(Role::Mask).is_none() {
            return Err(Error::MissingLayer("Mask".into()));
        }
        Ok(self.role_mask(Role::Mask, 0).not())
    }

    /// Built-in pattern ids, ascending.
    pub fn pattern_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .layers
            .iter()
            .filter_map(|l| pattern_id(&l.name))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// A built-in pattern: pure red / green / blue key the three colour slots.
    pub fn pattern(&self, id: u32) -> Result<RgbaImage> {
        self.layers
            .iter()
            .find(|l| pattern_id(&l.name) == Some(id))
            .map(|l| self.rgba_of(l))
            .ok_or_else(|| Error::MissingLayer(format!("car_pattern_{id:03}")))
    }

    fn spec_group(&self, channel: &str) -> Option<u32> {
        self.groups
            .iter()
            .find(|(_, (name, _))| normalise(name).starts_with(channel))
            .map(|(id, _)| *id)
    }

    /// Composite one spec channel group to a grey plane, bottom layer first, over black.
    fn spec_plane(&self, group: u32, parts: &mut Mask) -> Vec<u8> {
        let n = (self.width() * self.height()) as usize;
        let mut plane = vec![0u8; n];
        // psd lists layers top-first, so walk the group's layers in reverse.
        for layer in self
            .layers
            .iter()
            .rev()
            .filter(|l| self.psd.layers()[l.index].parent_id() == Some(group))
        {
            let rgba = self.rgba_of(layer);
            let is_parts = normalise(&layer.name) == "parts";
            for (i, p) in rgba.pixels().enumerate() {
                let [r, g, b, a] = p.0;
                if a == 0 {
                    continue;
                }
                if is_parts {
                    parts.data[i] = true;
                }
                // ITU-R 601 luma, as Pillow's convert("L").
                let luma =
                    ((r as u32 * 19595 + g as u32 * 38470 + b as u32 * 7471 + 0x8000) >> 16) as u8;
                let a = a as u32;
                plane[i] = ((luma as u32 * a + plane[i] as u32 * (255 - a) + 127) / 255) as u8;
            }
        }
        plane
    }

    /// The template's default spec map and hardware mask.
    pub fn spec_defaults(&self) -> SpecDefaults {
        let (w, h) = (self.width() as usize, self.height() as usize);
        let mut parts = Mask::new(w, h, false);
        let groups = [
            self.spec_group("red channel"),
            self.spec_group("green channel"),
            self.spec_group("blue channel"),
        ];
        if groups.iter().any(Option::is_none) {
            let rgb = FALLBACK_SPEC
                .iter()
                .copied()
                .cycle()
                .take(w * h * 3)
                .collect();
            return SpecDefaults {
                rgb,
                parts,
                from_template: false,
            };
        }
        let planes: Vec<Vec<u8>> = groups
            .iter()
            .map(|g| self.spec_plane(g.unwrap(), &mut parts))
            .collect();
        let mut rgb = vec![0u8; w * h * 3];
        for i in 0..w * h {
            rgb[i * 3] = planes[0][i];
            rgb[i * 3 + 1] = planes[1][i];
            rgb[i * 3 + 2] = planes[2][i];
        }
        SpecDefaults {
            rgb,
            parts,
            from_template: true,
        }
    }

    /// Summary used by the designer before it places anything.
    pub fn inspect(&self) -> Inspection {
        let total = (self.width() * self.height()) as usize;
        let paintable_px = self.paintable().map(|m| m.count()).unwrap_or(0);
        let spec = self.spec_defaults();
        let missing = [
            Role::Mask,
            Role::Wire,
            Role::NumberBlocks,
            Role::Mandatory,
            Role::SponsorBlocks,
            Role::Decal,
        ]
        .into_iter()
        .filter(|r| self.find_role(*r).is_none())
        .map(|r| r.label().to_string())
        .collect();
        Inspection {
            canvas: [self.width(), self.height()],
            layers: self.layers.clone(),
            pattern_ids: self.pattern_ids(),
            paintable_px,
            paintable_ratio: paintable_px as f64 / total as f64,
            number_block_boxes: component_boxes(&self.role_mask(Role::NumberBlocks, 0)),
            mandatory_boxes: component_boxes(&self.role_mask(Role::Mandatory, 0)),
            sponsor_block_boxes: component_boxes(&self.role_mask(Role::SponsorBlocks, 0)),
            decal_boxes: component_boxes(&self.role_mask(Role::Decal, 20)),
            has_spec_map: spec.from_template,
            hardware_parts_px: spec.parts.count(),
            missing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_normalised() {
        assert_eq!(normalise("Car_Mandatory"), "car mandatory");
        assert_eq!(normalise("Car  mandatory "), "car mandatory");
        assert_eq!(normalise("Number Boards"), "number boards");
    }

    #[test]
    fn pattern_ids_parse() {
        assert_eq!(pattern_id("car_pattern_012.tga"), Some(12));
        assert_eq!(pattern_id("Car Pattern 7"), Some(7));
        assert_eq!(pattern_id("car_pattern_.tga"), None);
        assert_eq!(pattern_id("car_pattern_01b.tga"), None);
        assert_eq!(pattern_id("Car Patterns"), None);
    }
}
