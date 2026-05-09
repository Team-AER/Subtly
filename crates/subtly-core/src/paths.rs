//! Filesystem path resolution: bundled assets, user-data models directory, binary names.

use directories::ProjectDirs;
use std::path::{Path, PathBuf};

const QUALIFIER: &str = "app";
const ORG: &str = "aer";
const APP: &str = "Subtly";

/// Locate the asset directory next to the executable, falling back to dev paths.
/// Mirrors `resolveAssetDir` in the original sidecar.
pub fn resolve_asset_dir() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("AER_ASSET_DIR") {
        let path = PathBuf::from(value);
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            // macOS app bundle: <app>.app/Contents/MacOS/<binary>
            // cargo-packager places resources at <app>.app/Contents/Resources/{bin,models}/...
            let macos_bundle = parent.join("../Resources");
            if macos_bundle.join("bin").exists() || macos_bundle.join("models").exists() {
                return Some(macos_bundle);
            }
            // Windows: cargo-packager places resources next to the .exe
            // Linux .deb / AppImage: same convention
            if parent.join("bin").exists() || parent.join("models").exists() {
                return Some(parent.to_path_buf());
            }
            // Legacy / dev fallback
            let resources = parent.join("resources").join("runtime").join("assets");
            if resources.exists() {
                return Some(resources);
            }
        }
    }

    let cwd = std::env::current_dir().ok()?;
    for candidate in [
        cwd.join("runtime/assets"),
        cwd.join("resources/runtime-assets"),
        cwd.join("assets"),
    ] {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// User data directory for downloaded models. Mirrors Electron's
/// `app.getPath('userData')/models`.
pub fn models_directory() -> PathBuf {
    if let Some(dirs) = ProjectDirs::from(QUALIFIER, ORG, APP) {
        let dir = dirs.data_dir().join("models");
        return dir;
    }
    // Last-resort fallback: cwd-relative.
    PathBuf::from("models")
}

pub fn ensure_models_directory() -> std::io::Result<PathBuf> {
    let dir = models_directory();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Configuration directory for persisted settings.
pub fn config_directory() -> PathBuf {
    if let Some(dirs) = ProjectDirs::from(QUALIFIER, ORG, APP) {
        return dirs.config_dir().to_path_buf();
    }
    PathBuf::from(".subtly")
}

#[cfg(windows)]
pub fn default_binary_name(base: &str) -> String {
    format!("{base}.exe")
}

#[cfg(not(windows))]
pub fn default_binary_name(base: &str) -> String {
    base.to_string()
}

/// Resolve a user-supplied path or fall back to bundled asset / generic default.
/// Mirrors `resolveOptionalPath` in the original sidecar.
pub fn resolve_optional_path(
    value: Option<&str>,
    asset_default: Option<PathBuf>,
    fallback: &str,
) -> String {
    if let Some(raw) = value {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(path) = asset_default {
        return path.to_string_lossy().to_string();
    }
    fallback.to_string()
}

pub fn ensure_path_exists(label: &str, path: &str) -> anyhow::Result<()> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("{label} not found at {path}"))
    }
}

pub fn ensure_executable_available(label: &str, path: &str) -> anyhow::Result<()> {
    let resolved = Path::new(path);
    let has_separator = path.contains(std::path::MAIN_SEPARATOR);
    if (resolved.is_absolute() || has_separator) && !resolved.exists() {
        return Err(anyhow::anyhow!("{label} not found at {path}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_optional_path_uses_value_when_set() {
        let p = resolve_optional_path(Some("/some/path"), None, "fallback");
        assert_eq!(p, "/some/path");
    }

    #[test]
    fn resolve_optional_path_uses_asset_when_value_blank() {
        let p = resolve_optional_path(
            Some("   "),
            Some(PathBuf::from("/asset")),
            "fallback",
        );
        assert_eq!(p, "/asset");
    }

    #[test]
    fn resolve_optional_path_uses_fallback_otherwise() {
        let p = resolve_optional_path(None, None, "fallback");
        assert_eq!(p, "fallback");
    }

    #[test]
    fn ensure_executable_handles_bare_name() {
        ensure_executable_available("test", "ffmpeg").unwrap();
    }
}
