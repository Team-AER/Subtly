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
//!
//! `notarytool submit --wait` exits 0 even when the result is `Invalid` — the
//! upload succeeded, only the *outcome* is bad. We must parse the JSON output,
//! check `status == "Accepted"`, and on anything else fetch `notarytool log`
//! to surface Apple's actual rejection reasons.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize)]
struct SubmitResponse {
    id: String,
    status: String,
    #[serde(default)]
    message: Option<String>,
}

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
    let output = Command::new("xcrun")
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
            "--output-format",
            "json",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        eprintln!("--- notarytool stdout ---\n{stdout}");
        eprintln!("--- notarytool stderr ---\n{stderr}");
        return Err(anyhow!("notarytool submit invocation failed"));
    }

    let resp: SubmitResponse = serde_json::from_str(stdout.trim()).map_err(|e| {
        anyhow!(
            "could not parse notarytool JSON ({e}). Raw stdout:\n{stdout}\nstderr:\n{stderr}"
        )
    })?;

    println!(
        "• notarytool result: id={} status={}{}",
        resp.id,
        resp.status,
        resp.message
            .as_deref()
            .map(|m| format!(" message={m}"))
            .unwrap_or_default()
    );

    if resp.status != "Accepted" {
        // Fetch Apple's per-issue log so the CI output shows exactly which file
        // failed and why. This is the diagnostic that was missing.
        eprintln!("• Fetching notarytool log for submission {}", resp.id);
        let log_out = Command::new("xcrun")
            .args([
                "notarytool",
                "log",
                &resp.id,
                "--apple-id",
                &apple_id,
                "--password",
                &pwd,
                "--team-id",
                &team,
            ])
            .output()?;
        eprintln!(
            "--- notarytool log ---\n{}\n{}",
            String::from_utf8_lossy(&log_out.stdout),
            String::from_utf8_lossy(&log_out.stderr)
        );
        return Err(anyhow!(
            "notarization rejected: status={} (id={})",
            resp.status,
            resp.id
        ));
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
