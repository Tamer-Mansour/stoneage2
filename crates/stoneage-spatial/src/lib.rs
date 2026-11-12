//! Area-wide views. OWNER: agent "spatial".
//!
//! * `ArealView` – port of `MessAreal$ViewArea/$ColHeader/$RowHeader` (horizontal depth-cut map of
//!   all rows × points, interpolation, block average, grid).
//! * `MapView`   – port of `MessKarte` + `$ViewArea` (background map image with the measurement area
//!   overlaid; move / scale / rotate handles stored in `survey.map`).
//! * `View3d`    – port of `Visual3d` + `ShapedTexture3D` (Java3D volume of stacked textured slices
//!   along X/Y/Z; orbit/zoom with the mouse; "recreate scenegraph"). Implemented with egui painting
//!   (software projection / egui meshes) – no Java3D, no extra GPU crate.
//!
//! CONTRACT: names below are used by stoneage-app while being written in parallel.
//! Add freely; do not rename/remove.

pub mod areal_view;
pub mod map_view;
pub mod view3d;

pub use areal_view::ArealView;
pub use map_view::MapView;
pub use stoneage_plots::ViewResponse;
pub use view3d::View3d;
