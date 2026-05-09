//! subtly-cli — minimal CLI for verifying core orchestration parity.
//!
//! Usage:
//!   subtly-cli ping
//!   subtly-cli list-devices
//!   subtly-cli smoke
//!   subtly-cli transcribe <input> [--output-dir <dir>] [--model <path>] [--vad <path>] [--dry-run]

use std::env;
use subtly_core::{
    devices::{list_devices, ping, smoke_test},
    events::Event,
    transcribe::{transcribe, TranscribeParams},
};
use tokio::sync::{mpsc, watch};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".to_string());
    match cmd.as_str() {
        "ping" => {
            let p = ping();
            println!("{}", serde_json::to_string_pretty(&p)?);
        }
        "list-devices" => {
            let d = list_devices();
            println!("{}", serde_json::to_string_pretty(&d)?);
        }
        "smoke" => match smoke_test() {
            Ok(msg) => println!("{msg}"),
            Err(e) => {
                eprintln!("smoke test failed: {e}");
                std::process::exit(1);
            }
        },
        "transcribe" => {
            let mut params = TranscribeParams {
                input_path: args.next().unwrap_or_default(),
                ..Default::default()
            };
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--output-dir" => params.output_dir = args.next(),
                    "--model" => params.model_path = args.next(),
                    "--vad" => params.vad_model_path = args.next(),
                    "--dry-run" => params.dry_run = Some(true),
                    other => eprintln!("ignoring unknown flag: {other}"),
                }
            }

            let (tx, mut rx) = mpsc::channel::<Event>(64);
            let (_cancel_tx, cancel_rx) = watch::channel(false);
            let logger = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    match event {
                        Event::Log(msg) => println!("[log] {msg}"),
                        Event::Progress(p) => println!("[progress] {p:?}"),
                        Event::Segment(s) => {
                            println!("[segment] {}-{} {}", s.start_ms, s.end_ms, s.text)
                        }
                        Event::OutputWritten(path) => println!("[output] {path}"),
                        Event::Done => println!("[done]"),
                    }
                }
            });

            let outcome = transcribe(params, tx, cancel_rx).await?;
            drop(logger);
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
        _ => {
            eprintln!(
                "subtly-cli — usage: ping | list-devices | smoke | transcribe <input> [flags]"
            );
            std::process::exit(2);
        }
    }
    Ok(())
}
