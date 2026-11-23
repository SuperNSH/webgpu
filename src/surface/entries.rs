//! The `#[native_components(<platform>)]` entries, re-exported at the
//! crate's root (`lib.rs`), where the hosts nsh generates look for them
//! (`app_<crate>::__nsh_native_components_<platform>`). Each platform file
//! defines its entry; this file re-exports them all already (an empty
//! module until it does, hence the `allow`).

#![allow(unused_imports, unreachable_pub)]

#[cfg(target_os = "android")]
pub use super::android::*;
#[cfg(target_os = "ios")]
pub use super::ios::*;
#[cfg(target_os = "linux")]
pub use super::linux::*;
#[cfg(target_os = "macos")]
pub use super::macos::*;
#[cfg(windows)]
pub use super::windows::*;
