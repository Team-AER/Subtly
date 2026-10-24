//! Subtly Iced UI entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod motion;
mod screens;
mod subtitle_preview;
mod theme;
mod updater;
mod widgets;

use app::App;

fn main() -> iced::Result {
    init_tracing();
    init_crash_reporting();
    enforce_single_instance();
    if !cfg!(debug_assertions) || !std::env::args().any(|arg| arg == "--ui-result-preview") {
        spawn_update_check();
    }

    let preview_small = cfg!(debug_assertions)
        && std::env::args().any(|arg| arg == "--ui-result-preview")
        && std::env::args().any(|arg| arg == "--ui-preview-small");
    iced::application("Subtly", App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .window_size(if preview_small {
            iced::Size::new(760.0, 640.0)
        } else {
            iced::Size::new(1100.0, 760.0)
        })
        .run_with(App::new)
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("subtly=info")),
        )
        .try_init();
}

#[cfg(feature = "crash-reporting")]
fn init_crash_reporting() {
    use std::sync::OnceLock;
    static GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();
    if let Ok(dsn) = std::env::var("SENTRY_DSN") {
        if !dsn.is_empty() {
            let _ = GUARD.set(sentry::init((
                dsn,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    traces_sample_rate: 0.2,
                    ..Default::default()
                },
            )));
        }
    }
}

#[cfg(not(feature = "crash-reporting"))]
fn init_crash_reporting() {}

fn enforce_single_instance() {
    use single_instance::SingleInstance;
    let instance = SingleInstance::new("app.aer.subtly").ok();
    if let Some(inst) = instance {
        if !inst.is_single() {
            eprintln!("Subtly is already running.");
            std::process::exit(0);
        }
        std::mem::forget(inst);
    }
    #[cfg(target_os = "windows")]
    set_app_user_model_id();
}

#[cfg(target_os = "windows")]
fn set_app_user_model_id() {
    // Best-effort; missing AppUserModelID just means worse taskbar grouping.
    // Real impl needs windows-sys. Punt for now.
}

fn spawn_update_check() {
    std::thread::spawn(|| {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return,
        };
        runtime.block_on(updater::check_for_updates());
    });
}
