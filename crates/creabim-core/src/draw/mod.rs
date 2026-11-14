//! 2D drawing: lines and shapes drawn in one view, as on a drawing board.
//! They are annotation: never in 3D, in schedules or exported as
//! building elements. One file per shape.

pub mod line;
pub mod polyline;

pub use line::DraftLine;
pub use polyline::{DraftPolyline, ShapeFill};
