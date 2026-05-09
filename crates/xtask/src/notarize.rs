//! macOS notarization via `xcrun notarytool` + `xcrun stapler`.
//!
//! Required environment:
//!   APPLE_ID                      — Apple ID email
//!   APPLE_APP_SPECIFIC_PASSWORD   — app-specific password from appleid.apple.com
//!   APPLE_TEAM_ID                 — 10-char Developer Team ID
//!
//! `notarytool submit` only accepts `.zip`, `.pkg`, or `.dmg`. When passed a
//! raw `.app` bundle we zip it to a temp file, submit the zip, then staple
//! the original `.app`. `.dmg` / `.pkg` / `.zip` are submitted as-is.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn notarize(input_path: &str) -> Result<()> {
    if !cfg!(target_os = "macos") {
        println!("• Skipping notarization (not macOS)");
        return Ok(());
    }

    let apple_id = std::env::var("APPLE_ID").ok();
    let pwd = std::env::var("APPLE_APP_SPECIFIC_PASSWORD").ok();
    let team = std::env::var("APPLE_TEAM_ID").ok();
    let (Some(apple_id), Some(pwd), Some(team)) = (apple_id, pwd, team) else {
        println!("• Skipping notarization (missing APPLE_ID / APPLE_APP_SPECIFIC_PASSWORD / APPLE_TEAM_ID)");
        return Ok(());
    };

    let path = Path::new(input_path);
    if !path.exists() {
        return Err(anyhow!("notarize input not found: {input_path}"));
    }

    let is_app = path.extension().and_then(|s| s.to_str()) == Some("app");
    let submit_path: PathBuf = if is_app {
        let zip = path.with_extension("app.zip");
        println!("• Zipping {input_path} → {}", zip.display());
        // ditto preserves resource forks / signatures correctly for notarization.
        let status = Command::new("ditto")
            .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
            .arg(path)
            .arg(&zip)
            .status()?;
        if !status.success() {
            return Err(anyhow!("ditto zip failed"));
        }
        zip
    } else {
        path.to_path_buf()
    };

    println!("• Submitting {} to notarytool", submit_path.display());
    let status = Command::new("xcrun")
        .args([
            "notarytool",
            "submit",
            submit_path.to_str().unwrap(),
            "--apple-id",
            &apple_id,
            "--password",
            &pwd,
            "--team-id",
            &team,
            "--wait",
        ])
        .status()?;
    if !status.success() {
        return Err(anyhow!("notarytool submit failed"));
    }

    // Staple the original artifact (the .app, .dmg, or .pkg). `.zip` cannot
    // be stapled — but in the .app-via-zip flow we staple the .app itself.
    let staple_target = if is_app { path } else { submit_path.as_path() };
    println!("• Stapling {}", staple_target.display());
    let status = Command::new("xcrun")
        .args(["stapler", "staple"])
        .arg(staple_target)
        .status()?;
    if !status.success() {
        return Err(anyhow!("stapler staple failed"));
    }

    println!("✓ Notarized {}", staple_target.display());
    Ok(())
}
