//! Auto-update via axoupdater. Reads cargo-dist's `dist-manifest.json`
//! published with each GitHub release.

#[cfg(feature = "auto-update")]
pub async fn check_for_updates() {
    use axoupdater::{AxoUpdater, ReleaseSource, ReleaseSourceType};
    let mut updater = AxoUpdater::new_for("subtly");
    updater.set_release_source(ReleaseSource {
        release_type: ReleaseSourceType::GitHub,
        owner: "prakhar1989".to_string(),
        name: "Subtly".to_string(),
        app_name: "subtly".to_string(),
    });
    if let Err(err) = updater.load_receipt() {
        tracing::debug!("axoupdater load_receipt: {err}");
        return;
    }
    match updater.is_update_needed().await {
        Ok(true) => {
            if let Err(err) = updater.run().await {
                tracing::warn!("auto-update failed: {err}");
            }
        }
        Ok(false) => {}
        Err(err) => tracing::debug!("axoupdater check failed: {err}"),
    }
}

#[cfg(not(feature = "auto-update"))]
pub async fn check_for_updates() {}
