//! Command-line front end for the render engine. Every command prints JSON on stdout.
//!
//!   liverai-engine inspect   --psd <template.psd>
//!   liverai-engine calibrate --psd <template.psd> --out <dir> --id <customer id>
//!   liverai-engine scan      --psd <template.psd> --boxes <boxes.json>
//!   liverai-engine render    --psd <template.psd> --livery <livery.json> --out <dir> --id <customer id>

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use liverai_engine::calibrate::calibrate;
use liverai_engine::islands::{Atlas, SegmentParams};
use liverai_engine::livery::Livery;
use liverai_engine::render::{render, RenderInput};
use liverai_engine::scan::{Keepouts, ScanBox};
use liverai_engine::template::Template;
use liverai_engine::{Error, Result};

#[derive(Parser)]
#[command(name = "liverai-engine", version, about = "Livery AI render engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Summarise a template: layers, patterns, paintable area and keep-out boxes.
    Inspect {
        #[arg(long)]
        psd: PathBuf,
    },
    /// Render the B0 calibration paint and islands.json.
    Calibrate {
        #[arg(long)]
        psd: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// iRacing customer id (or team id with --team).
        #[arg(long)]
        id: String,
        #[arg(long)]
        team: bool,
    },
    /// Test decal boxes against the template's keep-outs.
    Scan {
        #[arg(long)]
        psd: PathBuf,
        /// JSON array of {"name": ..., "bbox": [x0, y0, x1, y1]}.
        #[arg(long)]
        boxes: PathBuf,
    },
    /// Render a livery.json to the paint pair, a preview and lint.json.
    Render {
        #[arg(long)]
        psd: PathBuf,
        #[arg(long)]
        livery: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        team: bool,
        /// The car's calibrated panels.json.
        #[arg(long)]
        atlas: Option<PathBuf>,
    },
}

fn print(value: &impl serde::Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serialisable")
    );
}

fn run(cli: Cli) -> Result<bool> {
    match cli.command {
        Command::Inspect { psd } => {
            print(&Template::open(psd)?.inspect());
            Ok(true)
        }
        Command::Calibrate { psd, out, id, team } => {
            let template = Template::open(psd)?;
            let calibration = calibrate(&template, &SegmentParams::default())?;
            print(&calibration.write(&out, &id, team)?);
            Ok(true)
        }
        Command::Scan { psd, boxes } => {
            let template = Template::open(psd)?;
            let text = std::fs::read_to_string(&boxes)
                .map_err(|e| Error::Io(boxes.display().to_string(), e))?;
            let boxes: Vec<ScanBox> =
                serde_json::from_str(&text).map_err(|e| Error::Invalid(format!("boxes: {e}")))?;
            let results = Keepouts::from_template(&template)?.scan(&boxes);
            let clear = results.iter().all(|r| r.clear_for_fill);
            print(&results);
            Ok(clear)
        }
        Command::Render {
            psd,
            livery,
            out,
            id,
            team,
            atlas,
        } => {
            let template = Template::open(psd)?;
            let doc = Livery::load(&livery)?;
            let atlas = atlas.map(Atlas::load).transpose()?;
            let assets_dir = livery.parent().map(PathBuf::from).unwrap_or_default();
            let output = render(&RenderInput {
                template: &template,
                livery: &doc,
                assets_dir: &assets_dir,
                atlas: atlas.as_ref(),
            })?;
            let written = output.write(&out, &id, team)?;
            print(&written);
            Ok(written.lint.all_passed)
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        // Ran, but a check failed (lint or scan). The JSON on stdout has the detail.
        Ok(false) => ExitCode::from(2),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
