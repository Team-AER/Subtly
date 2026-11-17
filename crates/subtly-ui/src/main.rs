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
    let _crash_reporting = init_crash_reporting();
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
fn init_crash_reporting() -> Option<sentry::ClientInitGuard> {
    let dsn = std::env::var("GLITCHTIP_DSN")
        .or_else(|_| std::env::var("SENTRY_DSN"))
        .unwrap_or_else(|_| option_env!("GLITCHTIP_DSN").unwrap_or_default().to_owned());
    if dsn.trim().is_empty() {
        return None;
    }
    Some(sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(
                std::env::var("SENTRY_ENVIRONMENT")
                    .unwrap_or_else(|_| "production".into())
                    .into(),
            ),
            send_default_pii: false,
            auto_session_tracking: false,
            traces_sample_rate: 0.0,
            before_send: Some(std::sync::Arc::new(|mut event| {
                event.request = None;
                event.user = None;
                event.breadcrumbs.values.clear();
                event.extra.clear();
                for exception in event.exception.values.iter_mut() {
                    exception.value = Some(exception.ty.clone());
                }
                Some(event)
            })),
            ..Default::default()
        },
    )))
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

#[cfg(all(test, feature = "crash-reporting"))]
mod monitoring_tests {
    use super::init_crash_reporting;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    #[test]
    fn configured_panic_is_delivered_without_private_text() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let previous = std::env::var_os("GLITCHTIP_DSN");
        std::env::set_var("GLITCHTIP_DSN", format!("http://abcdef@{address}/1"));
        let receiver = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut buffer = [0; 4096];
                    loop {
                        let n = stream.read(&mut buffer).unwrap();
                        assert!(n > 0);
                        request.extend_from_slice(&buffer[..n]);
                        if let Some(split) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&request[..split]).to_lowercase();
                            let length: usize = headers
                                .lines()
                                .find_map(|line| line.strip_prefix("content-length:"))
                                .unwrap()
                                .trim()
                                .parse()
                                .unwrap();
                            if request.len() >= split + 4 + length {
                                break;
                            }
                        }
                    }
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                        )
                        .unwrap();
                    return String::from_utf8(request).unwrap();
                }
                assert!(
                    Instant::now() < deadline,
                    "Monitoring did not deliver the panic"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let guard = init_crash_reporting().expect("Configured SDK should start");
        let caught = std::panic::catch_unwind(|| panic!("PRIVATE_SENTINEL"));
        assert!(caught.is_err());
        drop(guard);
        match previous {
            Some(value) => std::env::set_var("GLITCHTIP_DSN", value),
            None => std::env::remove_var("GLITCHTIP_DSN"),
        }
        let request = receiver.join().unwrap();
        assert!(request.contains("\"exception\""));
        assert!(!request.contains("PRIVATE_SENTINEL"));
    }
}
