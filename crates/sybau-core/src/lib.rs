//! Core of V2Ray Sybau: everything that does not depend on the GUI.

pub mod config;
pub mod import;
pub mod logs;
pub mod model;
pub mod runtime;
pub mod settings;
pub mod share;
pub mod storage;
pub mod subscription;
pub mod testing;
pub mod updates;
pub mod util;

pub const APP_NAME: &str = "V2RaySybau";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "Xtop1337/v2raySYBAU";
