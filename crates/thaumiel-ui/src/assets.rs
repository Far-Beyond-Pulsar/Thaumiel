//! The compiled Next.js static export, baked into the binary. `web/out` is
//! produced by `build.rs` before this macro ever runs -- see that file.

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/out"]
pub struct Assets;
