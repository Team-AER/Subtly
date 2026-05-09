//! macOS notarization via `xcrun notarytool` + `xcrun stapler`.
//!
//! Required environment:
//!   APPLE_ID                      — Apple ID email
//!   APPLE_APP_SPECIFIC_PASSWORD   — app-specific password from appleid.apple.com
//!   APPLE_TEAM_ID                 — 10-char Developer Team ID

use anyhow::{anyhow, Result};
use std::process::Command;

pub fn notarize(app_path: &str) -> Result<()> {
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

    println!("• Submitting {app_path} to notarytool");
    let status = Command::new("xcrun")
        .args([
            "notarytool",
            "submit",
            app_path,
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

    println!("• Stapling {app_path}");
    let status = Command::new("xcrun")
        .args(["stapler", "staple", app_path])
        .status()?;
    if !status.success() {
        return Err(anyhow!("stapler staple failed"));
    }

    println!("✓ Notarized {app_path}");
    Ok(())
}
