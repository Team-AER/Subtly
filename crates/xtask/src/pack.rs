//! `xtask pack` — build release + run cargo-packager.

use anyhow::{anyhow, Result};
use std::process::Command;

pub fn pack() -> Result<()> {
    let status = Command::new("cargo")
        .args(["build", "--release", "-p", "subtly-ui"])
        .status()?;
    if !status.success() {
        return Err(anyhow!("cargo build failed"));
    }
    let status = Command::new("cargo")
        .args(["packager", "--release"])
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(anyhow!(
            "cargo-packager failed (install with `cargo install cargo-packager`)"
        )),
    }
}
