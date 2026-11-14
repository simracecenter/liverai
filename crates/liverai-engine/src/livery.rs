//! The declarative livery description (`livery.json`).
//!
//! Compatible with the proof-of-concept `livery/0.2` files: a recoloured built-in pattern,
//! per-panel colour and finish overrides, and decals placed on the UV canvas.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Material of a painted region, written to the spec map as R, G, B.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finish {
    /// 0 dielectric .. 255 metal.
    pub metallic: u8,
    /// 0 mirror .. 255 matte.
    pub roughness: u8,
    /// 0 none, 16 showroom; higher looks faded.
    pub clearcoat: u8,
}

impl Finish {
    pub fn triple(self) -> [f64; 3] {
        [
            self.metallic as f64,
            self.roughness as f64,
            self.clearcoat as f64,
        ]
    }
}

/// Finishes available without declaring them in the livery.
pub fn builtin_finishes() -> BTreeMap<String, Finish> {
    [
        (
            "gloss",
            Finish {
                metallic: 0,
                roughness: 28,
                clearcoat: 16,
            },
        ),
        (
            "satin",
            Finish {
                metallic: 0,
                roughness: 115,
                clearcoat: 16,
            },
        ),
        (
            "matte",
            Finish {
                metallic: 0,
                roughness: 215,
                clearcoat: 0,
            },
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatternRef {
    Id(u32),
    /// `car_pattern_012`, with or without `.tga`.
    Name(String),
}

impl PatternRef {
    pub fn id(&self) -> Result<u32> {
        match self {
            PatternRef::Id(id) => Ok(*id),
            PatternRef::Name(name) => {
                let digits: String = name
                    .trim_end_matches(".tga")
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                digits
                    .parse()
                    .map_err(|_| Error::Invalid(format!("pattern '{name}' has no number")))
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Slot {
    Colour(String),
    Full {
        colour: String,
        #[serde(default)]
        finish: Option<String>,
    },
}

impl Slot {
    pub fn colour(&self) -> &str {
        match self {
            Slot::Colour(c) | Slot::Full { colour: c, .. } => c,
        }
    }

    pub fn finish(&self) -> Option<&str> {
        match self {
            Slot::Colour(_) => None,
            Slot::Full { finish, .. } => finish.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Base {
    pub pattern: PatternRef,
    /// Keys `"1"`, `"2"`, `"3"`: slot 1 is the pattern colour with the largest paintable area.
    pub slots: BTreeMap<String, Slot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PanelFinish {
    pub panel: String,
    #[serde(default)]
    pub colour: Option<String>,
    #[serde(default)]
    pub finish: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decal {
    pub id: String,
    #[serde(default)]
    pub panel: Option<String>,
    /// PNG path relative to the livery file.
    pub asset: String,
    /// Centre on the UV canvas, in pixels.
    #[serde(default)]
    pub center_uv: Option<[f64; 2]>,
    /// Centre as a fraction of the panel's bounding box (needs `panel` and an atlas).
    #[serde(default)]
    pub center_panel: Option<[f64; 2]>,
    /// Width of the upright decal before rotation, in pixels.
    #[serde(default)]
    pub width_px: Option<f64>,
    /// Width as a fraction of the panel's extent along the decal's baseline.
    #[serde(default)]
    pub width_panel: Option<f64>,
    /// The panel's `up_angle_deg` from the atlas: 0 leaves the decal upright, 180 turns it
    /// upside down, 90 is a quarter turn counter-clockwise on the canvas and 270 a quarter
    /// turn clockwise. Defaults to the atlas value for `panel`.
    #[serde(default)]
    pub up_angle_deg: Option<i32>,
    #[serde(default)]
    pub flip_horizontal: bool,
    #[serde(default)]
    pub finish: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Livery {
    #[serde(default)]
    pub schema: Option<String>,
    pub name: String,
    pub car: String,
    pub base: Base,
    pub palette: BTreeMap<String, String>,
    #[serde(default)]
    pub finishes: BTreeMap<String, Finish>,
    /// Colour painted outside the paintable mask. Defaults to slot 1's colour.
    #[serde(default)]
    pub unpaintable_colour: Option<String>,
    #[serde(default)]
    pub panel_finishes: Vec<PanelFinish>,
    #[serde(default)]
    pub decals: Vec<Decal>,
}

pub fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let s = hex.strip_prefix('#')?;
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

impl Livery {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text =
            std::fs::read_to_string(path).map_err(|e| Error::Io(path.display().to_string(), e))?;
        Self::from_json(&text)
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let livery: Livery =
            serde_json::from_str(text).map_err(|e| Error::Invalid(format!("livery.json: {e}")))?;
        livery.validate()?;
        Ok(livery)
    }

    /// A palette name or a `#RRGGBB` literal.
    pub fn colour(&self, name: &str) -> Result<[u8; 3]> {
        let hex = self.palette.get(name).map(String::as_str).unwrap_or(name);
        parse_hex(hex).ok_or_else(|| Error::Invalid(format!("unknown colour '{name}'")))
    }

    /// A finish declared in the livery, or a built-in one. `None` means gloss.
    pub fn finish(&self, name: Option<&str>) -> Result<Finish> {
        let name = name.unwrap_or("gloss");
        self.finishes
            .get(name)
            .copied()
            .or_else(|| builtin_finishes().get(name).copied())
            .ok_or_else(|| Error::Invalid(format!("unknown finish '{name}'")))
    }

    /// Check every reference resolves, so a render fails before any pixel work.
    pub fn validate(&self) -> Result<()> {
        self.base.pattern.id()?;
        for key in ["1", "2", "3"] {
            let slot =
                self.base.slots.get(key).ok_or_else(|| {
                    Error::Invalid(format!("base.slots is missing slot \"{key}\""))
                })?;
            self.colour(slot.colour())?;
            self.finish(slot.finish())?;
        }
        if let Some(c) = &self.unpaintable_colour {
            self.colour(c)?;
        }
        for pf in &self.panel_finishes {
            if crate::islands::parse_panel_id(&pf.panel).is_none() {
                return Err(Error::Invalid(format!(
                    "panel_finishes: bad panel id '{}'",
                    pf.panel
                )));
            }
            if let Some(c) = &pf.colour {
                self.colour(c)?;
            }
            if pf.finish.is_some() {
                self.finish(pf.finish.as_deref())?;
            }
        }
        for d in &self.decals {
            self.finish(d.finish.as_deref())?;
            if d.center_uv.is_none() && d.center_panel.is_none() {
                return Err(Error::Invalid(format!(
                    "decal '{}' needs center_uv or center_panel",
                    d.id
                )));
            }
            if d.width_px.is_none() && d.width_panel.is_none() {
                return Err(Error::Invalid(format!(
                    "decal '{}' needs width_px or width_panel",
                    d.id
                )));
            }
            if let Some(a) = d.up_angle_deg {
                if ![0, 90, 180, 270].contains(&a) {
                    return Err(Error::Invalid(format!(
                        "decal '{}': up_angle_deg must be 0, 90, 180 or 270",
                        d.id
                    )));
                }
            }
            let asset = Path::new(&d.asset);
            if asset.is_absolute()
                || asset
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(Error::Invalid(format!(
                    "decal '{}': asset must be a relative path inside the project",
                    d.id
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r##"{
        "name": "t", "car": "c",
        "base": {"pattern": "car_pattern_012", "slots": {"1": "a", "2": {"colour": "#FF5F1F", "finish": "satin"}, "3": "a"}},
        "palette": {"a": "#05070A"}
    }"##;

    #[test]
    fn parses_poc_style_documents() {
        let l = Livery::from_json(MINIMAL).unwrap();
        assert_eq!(l.base.pattern.id().unwrap(), 12);
        assert_eq!(l.colour("a").unwrap(), [5, 7, 10]);
        assert_eq!(l.colour("#0EA5E9").unwrap(), [14, 165, 233]);
        assert_eq!(l.finish(Some("satin")).unwrap().roughness, 115);
        assert_eq!(l.finish(None).unwrap(), builtin_finishes()["gloss"]);
    }

    #[test]
    fn pattern_refs() {
        assert_eq!(PatternRef::Id(3).id().unwrap(), 3);
        assert_eq!(
            PatternRef::Name("car_pattern_017.tga".into()).id().unwrap(),
            17
        );
        assert!(PatternRef::Name("stripes".into()).id().is_err());
    }

    #[test]
    fn rejects_unknown_references_and_escaping_assets() {
        let bad_colour = MINIMAL.replace("\"1\": \"a\"", "\"1\": \"nope\"");
        assert!(Livery::from_json(&bad_colour).is_err());

        let missing_slot = MINIMAL.replace("\"3\": \"a\"", "\"4\": \"a\"");
        assert!(Livery::from_json(&missing_slot).is_err());

        let escaping = MINIMAL.replace(
            "\"palette\"",
            "\"decals\": [{\"id\": \"d\", \"asset\": \"../../secret.png\", \"center_uv\": [1, 1], \"width_px\": 10}], \"palette\"",
        );
        let err = Livery::from_json(&escaping).unwrap_err().to_string();
        assert!(err.contains("relative path"), "{err}");
    }

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(parse_hex("#ff5f1f"), Some([255, 95, 31]));
        assert_eq!(parse_hex("ff5f1f"), None);
        assert_eq!(parse_hex("#ff5f1"), None);
        assert_eq!(parse_hex("#ggggggg"), None);
    }
}
