//! Platform-independent core of Open-TmuxTabs.
//!
//! Everything in here is plain Rust without Windows APIs, so it can be
//! unit-tested on any host (`cargo test`). The Win32 user interface lives in
//! `crate::ui` and only uses these modules.

pub mod display;
pub mod i18n;
pub mod ini;
pub mod knownhosts;
pub mod model;
pub mod po;
pub mod protocol;
pub mod sshconfig;
pub mod validate;
pub mod wt;
