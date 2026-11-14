//! End-to-end tests against the synthetic template in `tests/fixtures`.
//!
//! `expected.json` was produced by `make_fixture.py` with psd-tools, NumPy and SciPy, using
//! the proof-of-concept's own code, so these tests compare the native engine with an
//! independent implementation.

use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage};
use liverai_engine::calibrate::calibrate;
use liverai_engine::islands::{segment, SegmentParams};
use liverai_engine::livery::Livery;
use liverai_engine::render::{blend_by_pattern, render, slot_order, RenderInput};
use liverai_engine::scan::{Keepouts, ScanBox};
use liverai_engine::template::{Role, Template};
use liverai_engine::tga;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn template() -> Template {
    Template::open(fixtures().join("synthetic_template.psd")).expect("fixture opens")
}

fn expected() -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixtures().join("expected.json")).unwrap())
        .unwrap()
}

fn boxes(v: &Value) -> Vec<[usize; 4]> {
    serde_json::from_value(v.clone()).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("liverai-engine-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const LIVERY: &str = r##"{
    "schema": "livery/1",
    "name": "fixture",
    "car": "synthetic",
    "base": {"pattern": 1, "slots": {
        "1": {"colour": "race_control", "finish": "gloss"},
        "2": {"colour": "apex_orange", "finish": "gloss"},
        "3": {"colour": "telemetry_blue", "finish": "satin"}}},
    "palette": {"race_control": "#05070A", "apex_orange": "#FF5F1F", "telemetry_blue": "#0EA5E9", "white": "#FFFFFF"},
    "finishes": {"chrome": {"metallic": 255, "roughness": 10, "clearcoat": 0}},
    "panel_finishes": [],
    "decals": []
}"##;

/// A livery with one decal; writes the decal PNG next to it and returns the assets directory.
fn livery_with_decal(dir: &Path, center: [f64; 2], width: f64) -> Livery {
    let mut png = RgbaImage::new(40, 20);
    for p in png.pixels_mut() {
        *p = Rgba([255, 255, 255, 255]);
    }
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    png.save(dir.join("assets/logo.png")).unwrap();
    let mut doc: Value = serde_json::from_str(LIVERY).unwrap();
    doc["decals"] = serde_json::json!([{
        "id": "logo", "asset": "assets/logo.png", "center_uv": center, "width_px": width,
        "up_angle_deg": 0, "finish": "chrome"
    }]);
    Livery::from_json(&doc.to_string()).unwrap()
}

#[test]
fn template_structure_matches_reference() {
    let t = template();
    let e = expected();
    assert_eq!([t.width(), t.height()], [512, 512]);
    assert_eq!(
        t.paintable().unwrap().count() as u64,
        e["paintable_px"].as_u64().unwrap()
    );
    assert_eq!(t.pattern_ids(), vec![0, 1]);

    let info = t.inspect();
    assert_eq!(info.number_block_boxes, boxes(&e["number_block_boxes"]));
    assert_eq!(info.mandatory_boxes, boxes(&e["mandatory_boxes"]));
    assert_eq!(
        info.hardware_parts_px as u64,
        e["parts_px"].as_u64().unwrap()
    );
    assert!(info.has_spec_map);
    assert!(info.missing.is_empty(), "{:?}", info.missing);
    assert_eq!(info.sponsor_block_boxes.len(), 1);
    assert_eq!(info.decal_boxes.len(), 1);

    let pattern = t
        .layers()
        .iter()
        .find(|l| l.name == "car_pattern_001.tga")
        .unwrap();
    assert!(!pattern.visible, "patterns are hidden in the fixture");
    assert_eq!(pattern.path, vec!["Paintable Area", "Car Patterns"]);
    let mask = t.layers().iter().find(|l| l.name == "Mask").unwrap();
    assert!(mask.visible);
    assert_eq!(mask.path, vec!["Turn Off Before Exporting TGA"]);
    assert_eq!(mask.bbox, [0, 0, 512, 512]);
}

#[test]
fn spec_defaults_composite_base_then_parts() {
    let t = template();
    let spec = t.spec_defaults();
    assert!(spec.from_template);
    let px = |x: usize, y: usize| {
        let i = (y * 512 + x) * 3;
        [spec.rgb[i], spec.rgb[i + 1], spec.rgb[i + 2]]
    };
    assert_eq!(px(10, 10), [0, 51, 0], "base paint");
    assert_eq!(px(470, 450), [200, 140, 0], "hardware parts");
    assert!(spec.parts.data[450 * 512 + 470]);
    assert!(!spec.parts.data[10 * 512 + 10]);
}

#[test]
fn pattern_recolour_is_bit_identical_to_reference() {
    let t = template();
    let e = expected();
    let paintable = t.paintable().unwrap();
    let pattern = t.pattern(1).unwrap();
    let order = slot_order(&pattern, &paintable);
    // Reference slot areas are [red, green, blue]; the largest takes the first colour.
    let areas: Vec<u64> = serde_json::from_value(e["pattern_001_slot_areas_px"].clone()).unwrap();
    let mut by_area = [0usize, 1, 2];
    by_area.sort_by_key(|i| std::cmp::Reverse(areas[*i]));
    assert_eq!(order, by_area);

    let colours = [[5.0, 7.0, 10.0], [255.0, 95.0, 31.0], [14.0, 165.0, 233.0]];
    let mut per_key = [[0.0; 3]; 3];
    for (rank, key) in order.iter().enumerate() {
        per_key[*key] = colours[rank];
    }
    let rgb: Vec<u8> = blend_by_pattern(&pattern, per_key)
        .into_iter()
        .map(|v| v.clamp(0.0, 255.0) as u8)
        .collect();
    let sha: String = Sha256::digest(&rgb)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(sha, e["pattern_001_recolour_sha256"].as_str().unwrap());
}

#[test]
fn islands_match_reference_segmentation() {
    let t = template();
    let e = expected();
    let islands = segment(
        &t.paintable().unwrap(),
        &t.role_rgba(Role::Wire).unwrap(),
        &SegmentParams::default(),
    );
    let reference = e["islands"].as_array().unwrap();
    assert_eq!(islands.count as usize, reference.len());

    let meta = islands.meta();
    // Two islands tie on area in the fixture, so compare as sets keyed by bounding box.
    for r in reference {
        let rb: [i64; 4] = serde_json::from_value(r["bbox"].clone()).unwrap();
        let ra = r["area_px"].as_i64().unwrap();
        let hit = meta.iter().find(|m| {
            m.bbox
                .iter()
                .zip(&rb)
                .all(|(a, b)| (*a as i64 - b).abs() <= 2)
        });
        let m = hit.unwrap_or_else(|| {
            panic!(
                "no island near {rb:?}; have {:?}",
                meta.iter().map(|m| m.bbox).collect::<Vec<_>>()
            )
        });
        let diff = (m.area_px as i64 - ra).abs() as f64 / ra as f64;
        assert!(
            diff < 0.01,
            "island {rb:?}: area {} vs reference {ra}",
            m.area_px
        );
    }
    // Ranked by area, largest first, and every paintable pixel belongs to an island.
    assert!(meta.windows(2).all(|w| w[0].area_px >= w[1].area_px));
    let labelled = islands.labels.data.iter().filter(|l| **l > 0).count();
    assert_eq!(labelled, t.paintable().unwrap().count());
    // The hood is one UV island split by a green panel line into two panels.
    let hood_halves = meta
        .iter()
        .filter(|m| m.bbox[1] >= 168 && m.bbox[3] <= 333 && m.bbox[2] <= 253)
        .count();
    assert_eq!(hood_halves, 2);
    for m in &meta {
        let (cx, cy) = (m.center[0], m.center[1]);
        assert_eq!(
            islands.labels.data[cy * 512 + cx],
            m.num,
            "centre lies inside its island"
        );
        assert!(m.max_inscribed_r > 20.0);
    }
}

#[test]
fn render_paints_slots_masks_and_hardware() {
    let t = template();
    let livery = Livery::from_json(LIVERY).unwrap();
    let out = render(&RenderInput {
        template: &t,
        livery: &livery,
        assets_dir: Path::new("."),
        atlas: None,
    })
    .unwrap();
    let diffuse = |x: usize, y: usize| {
        let i = (y * 512 + x) * 3;
        [out.diffuse[i], out.diffuse[i + 1], out.diffuse[i + 2]]
    };
    let spec = |x: usize, y: usize| {
        let i = (y * 512 + x) * 3;
        [out.spec[i], out.spec[i + 1], out.spec[i + 2]]
    };
    // Pattern 001: red key above y~200, green to 400, blue below. Red covers the most
    // paintable area, so it takes slot 1 (Race Control), green slot 2, blue slot 3.
    assert_eq!(
        diffuse(100, 150),
        [5, 7, 10],
        "red key inside side_right... slot 1"
    );
    assert_eq!(
        diffuse(200, 300),
        [255, 95, 31],
        "green key inside the hood: slot 2"
    );
    assert_eq!(
        diffuse(100, 480),
        [14, 165, 233],
        "blue key inside side_left: slot 3"
    );
    assert_eq!(spec(200, 300), [0, 28, 16], "gloss");
    assert_eq!(spec(100, 480), [0, 115, 16], "satin");
    // Outside the paintable mask: flat slot-1 colour.
    assert_eq!(diffuse(5, 5), [5, 7, 10]);
    assert_eq!(diffuse(300, 350), [5, 7, 10]);
    // Hardware keeps the template's material, inside and outside the islands.
    assert_eq!(spec(190, 250), [200, 140, 0]);
    assert_eq!(spec(470, 450), [200, 140, 0]);

    assert!(out.lint.format_ok);
    assert!(out.lint.hardware_parts_spec_preserved);
    assert_eq!(out.lint.keepout_number_blocks_px, 0);
    assert_eq!(out.lint.outside_paintable_px, 0);
    assert!(!out.lint.canvas_ok, "512 px is not an iRacing canvas size");
    assert!(!out.lint.all_passed);
}

#[test]
fn decals_sync_diffuse_and_spec_and_lint_flags_keepouts() {
    let t = template();
    let dir = temp_dir("decals");

    // Clear sheet metal on side_right.
    let clean = livery_with_decal(&dir, [180.0, 60.0], 40.0);
    let out = render(&RenderInput {
        template: &t,
        livery: &clean,
        assets_dir: &dir,
        atlas: None,
    })
    .unwrap();
    let i = (60 * 512 + 180) * 3;
    assert_eq!(&out.diffuse[i..i + 3], &[255, 255, 255]);
    assert_eq!(
        &out.spec[i..i + 3],
        &[255, 10, 0],
        "decal finish lands on the same pixels"
    );
    let d = &out.lint.decals[0];
    assert_eq!(d.bbox_uv, [160, 50, 200, 70]);
    assert_eq!(
        (
            d.keepout_number_blocks_px,
            d.keepout_mandatory_px,
            d.outside_paintable_px
        ),
        (0, 0, 0)
    );

    // Half over a number block (x 60..111, y 50..101).
    let over_number = livery_with_decal(&dir, [111.0, 60.0], 40.0);
    let out = render(&RenderInput {
        template: &t,
        livery: &over_number,
        assets_dir: &dir,
        atlas: None,
    })
    .unwrap();
    assert_eq!(out.lint.keepout_number_blocks_px, 20 * 20);
    assert!(!out.lint.all_passed);

    // Hanging off the island's edge into unpaintable space (island ends at x = 300).
    let off_edge = livery_with_decal(&dir, [300.0, 100.0], 40.0);
    let out = render(&RenderInput {
        template: &t,
        livery: &off_edge,
        assets_dir: &dir,
        atlas: None,
    })
    .unwrap();
    assert!(
        out.lint.outside_paintable_px > 300,
        "{}",
        out.lint.outside_paintable_px
    );
    // ...and the diffuse outside the mask is still the flat fill.
    let i = (100 * 512 + 310) * 3;
    assert_eq!(&out.diffuse[i..i + 3], &[5, 7, 10]);

    // Over the mandatory box (x 250..291, y 40..71).
    let over_mandatory = livery_with_decal(&dir, [270.0, 55.0], 40.0);
    let out = render(&RenderInput {
        template: &t,
        livery: &over_mandatory,
        assets_dir: &dir,
        atlas: None,
    })
    .unwrap();
    assert_eq!(out.lint.keepout_mandatory_px, 40 * 20);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn panel_finishes_recolour_one_island_only() {
    let t = template();
    let mut doc: Value = serde_json::from_str(LIVERY).unwrap();
    // p003 is the roof ellipse (third largest island).
    doc["panel_finishes"] =
        serde_json::json!([{"panel": "p003", "colour": "white", "finish": "matte"}]);
    let livery = Livery::from_json(&doc.to_string()).unwrap();
    let out = render(&RenderInput {
        template: &t,
        livery: &livery,
        assets_dir: Path::new("."),
        atlas: None,
    })
    .unwrap();
    let at = |x: usize, y: usize| {
        let i = (y * 512 + x) * 3;
        (
            [out.diffuse[i], out.diffuse[i + 1], out.diffuse[i + 2]],
            [out.spec[i], out.spec[i + 1], out.spec[i + 2]],
        )
    };
    assert_eq!(at(410, 270), ([255, 255, 255], [0, 215, 0]), "roof");
    assert_eq!(at(200, 300).0, [255, 95, 31], "hood untouched");

    doc["panel_finishes"] = serde_json::json!([{"panel": "p099", "colour": "white"}]);
    let livery = Livery::from_json(&doc.to_string()).unwrap();
    let err = render(&RenderInput {
        template: &t,
        livery: &livery,
        assets_dir: Path::new("."),
        atlas: None,
    });
    assert!(
        err.is_err(),
        "unknown panel is an error, not a silent no-op"
    );
}

#[test]
fn render_is_deterministic_and_files_round_trip() {
    let t = template();
    let dir = temp_dir("write");
    let livery = livery_with_decal(&dir, [180.0, 60.0], 40.0);
    let input = RenderInput {
        template: &t,
        livery: &livery,
        assets_dir: &dir,
        atlas: None,
    };
    let a = render(&input).unwrap();
    let b = render(&input).unwrap();
    assert_eq!(a.diffuse, b.diffuse);
    assert_eq!(a.spec, b.spec);

    let written = a.write(&dir.join("out"), "341237", false).unwrap();
    assert_eq!(written.files[0].name, "car_341237.tga");
    assert_eq!(written.files[1].name, "car_spec_341237.tga");
    let bytes = std::fs::read(dir.join("out/car_341237.tga")).unwrap();
    assert_eq!(bytes.len(), written.files[0].bytes);
    let (w, h, rgb) = tga::decode_rgb_rle(&bytes).unwrap();
    assert_eq!((w, h), (512, 512));
    assert_eq!(rgb, a.diffuse);
    assert!(dir.join("out/preview_uv.png").exists());
    let lint: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("out/lint.json")).unwrap()).unwrap();
    assert_eq!(lint["lint"]["format_ok"], true);

    assert!(a.write(&dir.join("out"), "../evil", false).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn calibration_paint_colours_every_island() {
    let t = template();
    let cal = calibrate(&t, &SegmentParams::default()).unwrap();
    assert_eq!(cal.islands.len(), 7);
    // Background is Cockpit Grey; an island interior away from its label is its calibration colour.
    assert_eq!(&cal.diffuse[0..3], &[17, 24, 39]);
    let roof = cal
        .islands
        .iter()
        .find(|m| m.bbox[0] >= 328 && m.bbox[1] >= 148 && m.bbox[3] <= 293)
        .unwrap();
    let hex = &roof.calibration_hex;
    let i = (220 * 512 + 345) * 3;
    assert_eq!(
        format!(
            "#{:02X}{:02X}{:02X}",
            cal.diffuse[i],
            cal.diffuse[i + 1],
            cal.diffuse[i + 2]
        ),
        *hex
    );
    // The id label draws white pixels near the island's centre.
    let (cx, cy) = (roof.center[0], roof.center[1]);
    let mut white = 0;
    for y in cy.saturating_sub(40)..cy + 40 {
        for x in cx.saturating_sub(40)..cx + 40 {
            let i = (y * 512 + x) * 3;
            if cal.diffuse[i..i + 3] == [255, 255, 255] {
                white += 1;
            }
        }
    }
    assert!(white > 50, "label drawn ({white} white px)");
    assert_eq!(cal.spec, t.spec_defaults().rgb);

    let dir = temp_dir("calibrate");
    let summary = cal.write(&dir, "341237", false).unwrap();
    assert_eq!(summary.island_count, 7);
    assert_eq!(summary.paintable_px, t.paintable().unwrap().count());
    assert!(dir.join("islands.json").exists());
    let header = tga::read_header(&std::fs::read(dir.join("car_341237.tga")).unwrap()).unwrap();
    assert_eq!((header.image_type, header.bits_per_pixel), (10, 24));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scan_reports_each_keepout() {
    let t = template();
    let keepouts = Keepouts::from_template(&t).unwrap();
    let b = |name: &str, bbox: [i64; 4]| ScanBox {
        name: name.into(),
        bbox,
    };
    let results = keepouts.scan(&[
        b("clean", [150, 40, 230, 110]),
        b("number", [100, 40, 140, 80]),
        b("mandatory", [240, 45, 280, 65]),
        b("off_island", [280, 100, 320, 125]),
        b("sample_decal", [385, 205, 430, 235]),
        b("off_canvas", [500, 40, 530, 60]),
        b("empty", [10, 10, 10, 10]),
    ]);
    let get = |n: &str| results.iter().find(|r| r.name == n).unwrap();
    assert!(get("clean").clear_for_text);
    // Box x 100..140, y 40..80 against the block at x 60..111, y 50..101.
    assert_eq!(get("number").number_blocks_px, 11 * 30);
    assert!(!get("number").clear_for_fill);
    assert_eq!(get("mandatory").mandatory_px, 30 * 20);
    assert!(get("off_island").clear_for_fill && !get("off_island").clear_for_text);
    assert!(get("off_island").unpaintable_px > 0);
    assert_eq!(get("sample_decal").car_decal_px, 45 * 30);
    assert!(!get("sample_decal").clear_for_text);
    assert_eq!(
        get("off_canvas").unpaintable_px,
        30 * 20,
        "pixels off the canvas count as unpaintable"
    );
    assert!(!get("empty").clear_for_fill);
}

#[test]
fn cli_inspect_prints_json_and_bad_input_fails() {
    let exe = env!("CARGO_BIN_EXE_liverai-engine");
    let out = std::process::Command::new(exe)
        .args(["inspect", "--psd"])
        .arg(fixtures().join("synthetic_template.psd"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["canvas"], serde_json::json!([512, 512]));
    assert_eq!(json["pattern_ids"], serde_json::json!([0, 1]));

    let out = std::process::Command::new(exe)
        .args(["inspect", "--psd", "does-not-exist.psd"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("does-not-exist.psd"));
}
