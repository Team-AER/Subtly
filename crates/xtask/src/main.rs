//! Subtly build helper. Replaces the Node-based `scripts/*.js`.
//!
//! Subcommands:
//!   xtask download-assets   — pull bundled binaries + VAD model (uses scripts/assets-manifest.json)
//!   xtask sync-assets       — copy resources/runtime-assets/ → runtime/assets
//!   xtask pack              — build release + run cargo-packager
//!   xtask notarize <path>   — submit a .app bundle for macOS notarization

mod assets;
mod notarize;
mod pack;

use anyhow::{anyhow, Result};
use std::env;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".to_string());
    match cmd.as_str() {
        "download-assets" => assets::download_assets(),
        "sync-assets" => assets::sync_assets(),
        "pack" => pack::pack(),
        "notarize" => {
            let path = args
                .next()
                .ok_or_else(|| anyhow!("usage: xtask notarize <path-to-.app>"))?;
            notarize::notarize(&path)
        }
        _ => {
            eprintln!("xtask: download-assets | sync-assets | pack | notarize");
            std::process::exit(2);
        }
    }
}
