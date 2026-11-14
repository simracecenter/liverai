//! Bundled car library: `resources/cars/index.json` plus one
//! `<car>/panels.json` panel atlas per car.

use crate::error::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const INDEX_FILE: &str = "index.json";
const PANELS_FILE: &str = "panels.json";

#[derive(Debug, Deserialize)]
struct Index {
    cars: Vec<IndexEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct IndexEntry {
    id: String,
    name: String,
}

/// A panel atlas as written by calibration (`panels/0.1`).
#[derive(Debug, Deserialize)]
struct Atlas {
    schema: String,
    canvas: [u32; 2],
    total_paintable_px: u64,
    confirmed_paintable_px: u64,
    confirmed_coverage_ratio: f64,
    #[serde(default)]
    paint_folder: Option<String>,
    panels: Vec<AtlasPanel>,
}

#[derive(Debug, Deserialize)]
struct AtlasPanel {
    id: String,
    bbox: [u32; 4],
    area_px: u64,
    #[serde(default)]
    calibration_hex: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    up_angle_deg: Option<i32>,
    #[serde(default)]
    forward: Option<String>,
    #[serde(default)]
    mirror_of: Option<String>,
    confidence: String,
    #[serde(default)]
    evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CarSummary {
    pub id: String,
    pub name: String,
    pub canvas: [u32; 2],
    pub panel_count: usize,
    pub confirmed_panel_count: usize,
    /// Share of paintable area with a confirmed panel label, 0.0 to 1.0.
    pub confirmed_coverage_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Panel {
    pub id: String,
    pub label: Option<String>,
    pub bbox: [u32; 4],
    pub area_px: u64,
    pub calibration_hex: Option<String>,
    pub up_angle_deg: Option<i32>,
    pub forward: Option<String>,
    pub mirror_of: Option<String>,
    pub confidence: String,
    pub evidence_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CarDetail {
    #[serde(flatten)]
    pub summary: CarSummary,
    pub atlas_schema: String,
    /// Folder name under the iRacing paint folder.
    pub paint_folder: String,
    pub total_paintable_px: u64,
    pub confirmed_paintable_px: u64,
    pub panels: Vec<Panel>,
}

fn load_index(cars_dir: &Path) -> CommandResult<Vec<IndexEntry>> {
    let text = fs::read_to_string(cars_dir.join(INDEX_FILE))
        .map_err(|e| CommandError::io("Read car index", &e))?;
    let index: Index =
        serde_json::from_str(&text).map_err(|e| CommandError::data("Parse car index", &e))?;
    Ok(index.cars)
}

fn load_atlas(cars_dir: &Path, id: &str) -> CommandResult<Atlas> {
    let text = fs::read_to_string(cars_dir.join(id).join(PANELS_FILE))
        .map_err(|e| CommandError::io(&format!("Read panel atlas for {id}"), &e))?;
    serde_json::from_str(&text)
        .map_err(|e| CommandError::data(&format!("Parse panel atlas for {id}"), &e))
}

fn is_confirmed(panel: &AtlasPanel) -> bool {
    panel.confidence == "confirmed"
}

fn summarise(entry: &IndexEntry, atlas: &Atlas) -> CarSummary {
    CarSummary {
        id: entry.id.clone(),
        name: entry.name.clone(),
        canvas: atlas.canvas,
        panel_count: atlas.panels.len(),
        confirmed_panel_count: atlas.panels.iter().filter(|p| is_confirmed(p)).count(),
        confirmed_coverage_ratio: atlas.confirmed_coverage_ratio,
    }
}

/// All bundled cars, in index order.
pub fn list(cars_dir: &Path) -> CommandResult<Vec<CarSummary>> {
    load_index(cars_dir)?
        .iter()
        .map(|entry| load_atlas(cars_dir, &entry.id).map(|atlas| summarise(entry, &atlas)))
        .collect()
}

/// True when `id` names a car in the bundled index.
pub fn exists(cars_dir: &Path, id: &str) -> CommandResult<bool> {
    Ok(load_index(cars_dir)?.iter().any(|e| e.id == id))
}

/// One car with its panel atlas. `id` is only ever matched against the index,
/// never used as a path taken from the caller.
pub fn get(cars_dir: &Path, id: &str) -> CommandResult<CarDetail> {
    let entry = load_index(cars_dir)?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| CommandError::not_found("Car is not in the library."))?;
    let atlas = load_atlas(cars_dir, &entry.id)?;
    let summary = summarise(&entry, &atlas);
    Ok(CarDetail {
        summary,
        atlas_schema: atlas.schema,
        paint_folder: atlas.paint_folder.unwrap_or_else(|| entry.id.clone()),
        total_paintable_px: atlas.total_paintable_px,
        confirmed_paintable_px: atlas.confirmed_paintable_px,
        panels: atlas
            .panels
            .into_iter()
            .map(|p| Panel {
                evidence_count: p.evidence.len(),
                id: p.id,
                label: p.label,
                bbox: p.bbox,
                area_px: p.area_px,
                calibration_hex: p.calibration_hex,
                up_angle_deg: p.up_angle_deg,
                forward: p.forward,
                mirror_of: p.mirror_of,
                confidence: p.confidence,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The real bundled library in the repo.
    fn bundled() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/cars")
    }

    #[test]
    fn bundled_library_lists_eleven_calibrated_cars() {
        let cars = list(&bundled()).unwrap();
        assert_eq!(cars.len(), 11);
        assert_eq!(cars[0].id, "porsche992rgt3");
        assert_eq!(cars[0].name, "Porsche 911 GT3 R (992)");
        for car in &cars {
            assert_eq!(car.canvas, [2048, 2048], "{}", car.id);
            assert!(car.panel_count > 0, "{}", car.id);
            assert!(car.confirmed_panel_count <= car.panel_count, "{}", car.id);
            assert!(
                (0.8..=1.0).contains(&car.confirmed_coverage_ratio),
                "{} coverage {}",
                car.id,
                car.confirmed_coverage_ratio
            );
        }
    }

    #[test]
    fn bundled_car_detail_matches_its_atlas() {
        let car = get(&bundled(), "porsche992rgt3").unwrap();
        assert_eq!(car.summary.panel_count, 45);
        assert_eq!(car.panels.len(), 45);
        assert_eq!(car.atlas_schema, "panels/0.1");
        assert_eq!(car.paint_folder, "porsche992rgt3");
        let p1 = &car.panels[0];
        assert_eq!(p1.id, "p001");
        assert_eq!(p1.label.as_deref(), Some("body_side_right"));
        assert_eq!(p1.mirror_of.as_deref(), Some("p002"));
        assert_eq!(p1.up_angle_deg, Some(180));
        assert!(car.confirmed_paintable_px <= car.total_paintable_px);

        // The atlas names the paint folder when it differs from the id.
        let mx5 = get(&bundled(), "mx5_mx52016").unwrap();
        assert_eq!(mx5.paint_folder, "mx5 mx52016");
    }

    #[test]
    fn every_bundled_car_loads() {
        for car in list(&bundled()).unwrap() {
            let detail = get(&bundled(), &car.id).unwrap();
            assert_eq!(detail.summary, car);
        }
    }

    #[test]
    fn unknown_and_traversal_ids_are_not_found() {
        for id in [
            "",
            "nope",
            "../cars/porsche992rgt3",
            "porsche992rgt3/..",
            ".",
        ] {
            let err = get(&bundled(), id).unwrap_err();
            assert_eq!(err.code, "not_found", "{id:?}");
            assert!(!exists(&bundled(), id).unwrap(), "{id:?}");
        }
        assert!(exists(&bundled(), "ferrari296gt3").unwrap());
    }

    #[test]
    fn missing_library_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(list(dir.path()).unwrap_err().code, "io");
    }
}
