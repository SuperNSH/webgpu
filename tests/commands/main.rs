//! The commands area (command encoder, passes, bundles, queue, image
//! bitmaps, external textures), called from Rust as TSX calls it, on the
//! machine's GPU. Images are checked against react-native-webgpu's own
//! snapshots (`examples/headless/snapshots/commands/`) with its tolerance
//! (pixelmatch, threshold 0.1, at most 200 different pixels).
//!
//! Tests that need what area (a) resources has not built yet (bind groups,
//! samplers, compute pipelines, query sets) are `#[ignore]`d with
//! "blocked on area (a)"; `src/api/compute_pass_encoder.rs` covers the
//! compute pass with wgpu objects made directly in the meantime.

mod bundles;
mod common;
mod compute;
mod copies;
mod draws;
mod encoder;
mod external;
mod image_bitmap;
mod pixelmatch;
mod queue;
