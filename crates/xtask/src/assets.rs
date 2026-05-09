//! Asset download + sync. Ports `scripts/download-assets.js` and `scripts/sync-assets.js`.

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Manifest {
    platforms: BTreeMap<String, Vec<Asset>>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    url: String,
    sha256: String,
    dest: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    extract: Option<String>,
}

fn repo_root() -> PathBuf {
    // crates/xtask/src/main.rs → up 3 = repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .expect("repo root")
}

fn current_platform_key() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "darwin-arm64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "darwin-x64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "win32-x64"
    } else {
        "unknown"
    }
}

pub fn download_assets() -> Result<()> {
    let root = repo_root();
    let manifest_path = root.join("scripts/assets-manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let manifest: Manifest = serde_json::from_str(&manifest_text)?;

    let key = current_platform_key();
    let assets = manifest
        .platforms
        .get(key)
        .ok_or_else(|| anyhow!("no manifest entry for platform {key}"))?;

    println!("• Downloading {} asset(s) for {key}", assets.len());
    for asset in assets {
        let dest = root.join(&asset.dest);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let final_path = match asset.extract.as_deref() {
            Some("gunzip") => dest.with_extension(""),
            _ => dest.clone(),
        };
        // Cached check uses the on-disk archive when extract is set, else the dest itself.
        let archive_for_check = match asset.extract.as_deref() {
            Some("gunzip") => dest.clone(),
            _ => dest.clone(),
        };
        if final_path.exists()
            && archive_for_check.exists()
            && verify_sha256(&archive_for_check, &asset.sha256).unwrap_or(false)
        {
            println!("  ✓ {} (cached)", asset.name);
            continue;
        }

        println!("  ↓ {}", asset.name);
        let body = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?
            .get(&asset.url)
            .send()?
            .error_for_status()?
            .bytes()?;
        let bytes: Vec<u8> = body.to_vec();

        // Verify the downloaded bytes (the manifest hash is of the wire payload).
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != asset.sha256 {
            return Err(anyhow!(
                "sha256 mismatch for {}: expected {}, got {}",
                asset.name,
                asset.sha256,
                actual_hash
            ));
        }

        // Persist the archive (so cache check can re-verify on next run).
        fs::write(&dest, &bytes)?;

        // Extract if needed.
        if matches!(asset.extract.as_deref(), Some("gunzip")) {
            let mut decoder = GzDecoder::new(bytes.as_slice());
            let mut decoded = Vec::new();
            decoder.read_to_end(&mut decoded)?;
            let mut f = fs::File::create(&final_path)?;
            f.write_all(&decoded)?;
        }

        if let Some(mode) = &asset.mode {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let bits = u32::from_str_radix(mode, 8)?;
                fs::set_permissions(&final_path, fs::Permissions::from_mode(bits))?;
            }
            #[cfg(not(unix))]
            let _ = mode;
        }
        println!("  ✓ {}", asset.name);
    }
    Ok(())
}

fn verify_sha256(path: &Path, expected: &str) -> Result<bool> {
    let bytes = fs::read(path)?;
    Ok(sha256_hex(&bytes) == expected)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sync_assets() -> Result<()> {
    let root = repo_root();
    let src = root.join("resources/runtime-assets");
    let dst = root.join("runtime/assets");
    if !src.exists() {
        return Err(anyhow!("source missing: {}", src.display()));
    }
    fs::create_dir_all(&dst)?;
    copy_dir(&src, &dst)?;
    println!("• Synced {} → {}", src.display(), dst.display());
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            fs::create_dir_all(&to)?;
            copy_dir(&from, &to)?;
        } else {
            let mut input = fs::File::open(&from)?;
            let mut output = fs::File::create(&to)?;
            copy(&mut input, &mut output)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = from.metadata()?.permissions().mode();
                fs::set_permissions(&to, fs::Permissions::from_mode(mode))?;
            }
        }
    }
    Ok(())
}
