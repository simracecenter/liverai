//! Native render engine for Livery AI.
//!
//! Reads an iRacing paint template (`.psd`), segments its UV islands, renders a declarative
//! livery to the paint pair iRacing loads (24-bit RLE `car_<id>.tga` and `car_spec_<id>.tga`)
//! and lints the result. No network access and no platform-specific code.

// Pixel loops index several parallel buffers; explicit indices read more clearly there.
#![allow(clippy::needless_range_loop)]

pub mod calibrate;
pub mod islands;
pub mod livery;
pub mod raster;
pub mod render;
pub mod scan;
pub mod template;
pub mod tga;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(String, #[source] std::io::Error),
    #[error("template: {0}")]
    Template(String),
    #[error("template has no '{0}' layer")]
    MissingLayer(String),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The engine's version, shown in the app's About panel.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
