//! Workspace screen — the primary "transcribe a file" surface.

use iced::widget::{
    button, checkbox, column, container, progress_bar, row, text, text_input, Column, Row, Space,
};
use iced::{Alignment, Element, Length};

use crate::app::{App, Message, Screen, TranscribeResult};
use crate::theme as t;

pub fn view<'a>(app: &'a App) -> Element<'a, Message> {
    let s = &app.settings;
    let busy = app.is_transcribing;

    let mut pick_file_btn = button(text("Pick file".to_string()).size(13))
        .padding([8, 14])
        .style(t::secondary_button);
    let mut pick_dir_btn = button(text("Pick folder".to_string()).size(13))
        .padding([8, 14])
        .style(t::secondary_button);
    if !busy {
        pick_file_btn = pick_file_btn.on_press(Message::PickFile);
        pick_dir_btn = pick_dir_btn.on_press(Message::PickDir);
    }
    let pickers = row![pick_file_btn, pick_dir_btn].spacing(8);

    let input_display: Element<'_, Message> = if s.input_path.is_empty() {
        text("No input selected".to_string())
            .size(13)
            .color(t::TEXT_FAINT)
            .into()
    } else {
        text(s.input_path.clone()).size(13).color(t::TEXT).into()
    };

    let input_block = section(
        "Input",
        "Pick a media file or a folder to batch-transcribe.",
        column![
            pickers,
            container(input_display)
                .style(t::surface_inset)
                .padding([10, 14])
                .width(Length::Fill),
        ]
        .spacing(10)
        .into(),
    );

    let mut output_input = text_input("Defaults to input file/folder", &s.output_dir)
        .padding(10)
        .style(t::text_input_style);
    if !busy {
        output_input = output_input.on_input(Message::SetOutputDir);
    }
    let output_block = section(
        "Output",
        "Optional. Leave blank to write next to the input file.",
        column![
            output_input,
            text("Export formats").size(13).color(t::TEXT),
            format_selector(s, busy),
        ]
        .spacing(12)
        .into(),
    );

    let prompt_block = section(
        "Custom vocabulary",
        "Optional. Paste names, brands, or jargon you want spelled correctly. Whisper biases the transcript toward this text.",
        prompt_input(s, busy),
    );

    let replacements_block = section(
        "Find & replace",
        "Optional. Fix recurring mistranscriptions across every output format.",
        replacements_view(s, busy),
    );

    // Model & device summary
    let whisper_ok = s
        .selected_model
        .as_deref()
        .map(|id| {
            app.installed
                .iter()
                .any(|m| m.descriptor.id == id && m.complete)
        })
        .unwrap_or(false);
    let vad_ok = app
        .installed
        .iter()
        .any(|m| m.descriptor.id == "silero-vad" && m.complete);
    let model_label = match (&s.selected_model, whisper_ok) {
        (Some(id), true) => format!("Model: {id}"),
        (Some(id), false) => format!("Model {id} not installed"),
        (None, _) => "No model selected".to_string(),
    };
    let device_label = app
        .selected_device
        .as_ref()
        .map(|d| format!("Device: {} ({})", d.name, d.backend))
        .unwrap_or_else(|| "No device".to_string());

    let mut chips_row: Row<'_, Message> = Row::new().spacing(8);
    chips_row = chips_row.push(chip(&model_label, whisper_ok));
    chips_row = chips_row.push(chip(
        if vad_ok {
            "VAD: installed"
        } else {
            "VAD: missing"
        },
        vad_ok,
    ));
    chips_row = chips_row.push(chip(&device_label, app.selected_device.is_some()));
    if let Some(d) = &app.detected_language {
        // Surface what Whisper actually picked. With language=auto this is
        // the detection result; with an explicit code it's the echo. Either
        // way it's the line that turns "did Whisper hear me right?" into a
        // clear yes/no the user can spot at a glance.
        chips_row = chips_row.push(chip(&format!("Detected: {} ({})", d.name, d.code), true));
    }
    let context_chips = chips_row.wrap();

    // Action area: either a Generate button + helper, or an inline progress card.
    let action_area: Element<'_, Message> = if let Some(progress) = &app.transcribe_progress {
        progress_card(progress, &s.input_path)
    } else {
        let mut transcribe = button(text("Generate subtitles".to_string()).size(14))
            .padding([12, 22])
            .style(t::primary_button);
        if !s.input_path.is_empty() && whisper_ok && vad_ok {
            transcribe = transcribe.on_press(Message::StartTranscribe);
        }

        let mut helper: Option<Element<'_, Message>> = None;
        if !whisper_ok || !vad_ok {
            helper = Some(
                row![
                    text("⚠".to_string()).size(13).color(t::WARN),
                    text("Models missing — open the Models tab to download.".to_string())
                        .size(13)
                        .color(t::TEXT_MUTED),
                    Space::with_width(Length::Fixed(8.0)),
                    button(text("Open Models".to_string()).size(12))
                        .padding([6, 12])
                        .on_press(Message::NavigateTo(Screen::Models))
                        .style(t::ghost_button),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );
        } else if s.input_path.is_empty() {
            helper = Some(
                text("Select an input to enable subtitle generation.".to_string())
                    .size(12)
                    .color(t::TEXT_MUTED)
                    .into(),
            );
        }

        let mut action_col = column![transcribe].spacing(8);
        if let Some(h) = helper {
            action_col = action_col.push(h);
        }
        action_col.into()
    };

    let mut content = column![input_block, output_block, context_chips,]
        .spacing(20)
        .max_width(820.0);
    if let Some(result) = &app.transcribe_result {
        content = content.push(result_card(result));
    }
    content = content.push(action_area).push(
        button(text(if app.workspace_refinements_expanded {
            "▾ Vocabulary & replacements"
        } else {
            "▸ Vocabulary & replacements"
        }))
        .on_press(Message::ToggleWorkspaceRefinements)
        .padding([10, 14])
        .style(t::secondary_button),
    );
    if app.workspace_refinements_expanded {
        content = content.push(prompt_block).push(replacements_block);
    }
    content.into()
}

fn prompt_input<'a>(s: &subtly_core::settings::Settings, busy: bool) -> Element<'a, Message> {
    let mut input = text_input(
        "e.g. Areca palm, Subtly, Iced. Names and jargon Whisper might miss.",
        &s.initial_prompt,
    )
    .padding(10)
    .style(t::text_input_style);
    if !busy {
        input = input.on_input(Message::SetInitialPrompt);
    }
    input.into()
}

fn replacements_view<'a>(s: &subtly_core::settings::Settings, busy: bool) -> Element<'a, Message> {
    let mut col: Column<'_, Message> = Column::new().spacing(8);
    if s.replacements.is_empty() {
        col = col.push(
            text("No rules yet. Add one to fix a recurring transcription error.".to_string())
                .size(11)
                .color(t::TEXT_FAINT),
        );
    } else {
        for (idx, rule) in s.replacements.iter().enumerate() {
            let mut from_input = text_input("Find (e.g. Eryka palm)", &rule.from)
                .padding(8)
                .style(t::text_input_style);
            let mut to_input = text_input("Replace with (e.g. Areca palm)", &rule.to)
                .padding(8)
                .style(t::text_input_style);
            if !busy {
                from_input = from_input.on_input(move |v| Message::SetReplacementFrom(idx, v));
                to_input = to_input.on_input(move |v| Message::SetReplacementTo(idx, v));
            }
            let mut remove_btn = button(text("Remove".to_string()).size(11))
                .padding([6, 10])
                .style(t::ghost_button);
            if !busy {
                remove_btn = remove_btn.on_press(Message::RemoveReplacement(idx));
            }
            let case_box = checkbox("Case", rule.case_sensitive)
                .size(13)
                .text_size(11)
                .on_toggle(move |v| Message::ToggleReplacementCaseSensitive(idx, v));
            let word_box = checkbox("Whole word", rule.whole_word)
                .size(13)
                .text_size(11)
                .on_toggle(move |v| Message::ToggleReplacementWholeWord(idx, v));
            let row = column![
                row![
                    container(from_input).width(Length::FillPortion(1)),
                    container(to_input).width(Length::FillPortion(1)),
                ]
                .spacing(8),
                row![
                    case_box,
                    word_box,
                    Space::with_width(Length::Fill),
                    remove_btn,
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .spacing(6);
            col = col.push(container(row).style(t::surface_inset).padding(10));
        }
    }
    let mut add_btn = button(text("Add rule".to_string()).size(12))
        .padding([8, 14])
        .style(t::secondary_button);
    if !busy {
        add_btn = add_btn.on_press(Message::AddReplacement);
    }
    col.push(add_btn).into()
}

fn result_card(result: &TranscribeResult) -> Element<'_, Message> {
    let activity = button(text("View activity").size(12))
        .padding([6, 12])
        .on_press(Message::NavigateTo(Screen::Logs))
        .style(t::ghost_button);
    let (title, detail, primary) = match result {
        TranscribeResult::Completed {
            jobs,
            outputs,
            dry_run,
        } => {
            let folders: std::collections::BTreeSet<_> = outputs
                .iter()
                .filter_map(|path| std::path::Path::new(path).parent())
                .collect();
            let (title, detail) = completion_copy(*jobs, outputs, *dry_run);
            let primary = if *dry_run || outputs.is_empty() {
                None
            } else {
                folders.iter().next().map(|folder| {
                    button(text("Open output folder").size(12))
                        .padding([8, 14])
                        .on_press(Message::OpenOutputFolder(
                            folder.to_string_lossy().into_owned(),
                        ))
                        .style(t::primary_button)
                })
            };
            (title, detail, primary)
        }
        TranscribeResult::Failed(error) => (
            "Subtitle generation failed".to_string(),
            error.clone(),
            Some(
                button(text("Retry").size(12))
                    .padding([8, 14])
                    .on_press(Message::StartTranscribe)
                    .style(t::primary_button),
            ),
        ),
        TranscribeResult::Cancelled => (
            "Subtitle generation cancelled".to_string(),
            "Any files already written remain in the output folder. See activity for details."
                .to_string(),
            None,
        ),
    };
    let mut actions = row![].spacing(8).align_y(Alignment::Center);
    if let Some(primary) = primary {
        actions = actions.push(primary);
    }
    actions = actions.push(activity);
    container(
        column![
            text(title).size(15),
            text(detail).size(13).color(t::TEXT_MUTED),
            actions
        ]
        .spacing(10),
    )
    .style(t::surface)
    .padding(18)
    .width(Length::Fill)
    .into()
}

/// Completion wording follows the dispatched run, including dry runs which
/// return planned paths without writing files.
fn completion_copy(jobs: usize, outputs: &[String], dry_run: bool) -> (String, String) {
    let formats = outputs
        .iter()
        .filter_map(|path| std::path::Path::new(path).extension()?.to_str())
        .map(str::to_uppercase)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" · ");
    let formats = if formats.is_empty() {
        String::new()
    } else {
        format!("Formats: {formats}. ")
    };
    if dry_run {
        return (
            format!(
                "Dry run complete · {jobs} job(s) · {} planned files",
                outputs.len()
            ),
            format!("{formats}No files were written. View activity for planned paths."),
        );
    }
    if outputs.is_empty() {
        return (
            "Finished without output files".to_string(),
            "No subtitle files were written. View activity for details.".to_string(),
        );
    }
    let folders = outputs
        .iter()
        .filter_map(|path| std::path::Path::new(path).parent())
        .collect::<std::collections::BTreeSet<_>>();
    let destination = if folders.len() == 1 {
        format!("Saved in {}", folders.iter().next().unwrap().display())
    } else {
        format!(
            "Saved across {} folders. See activity for every output path.",
            folders.len()
        )
    };
    (
        format!("Subtitles ready · {jobs} job(s) · {} files", outputs.len()),
        format!("{formats}{destination}"),
    )
}

#[cfg(test)]
mod result_tests {
    use super::completion_copy;

    #[test]
    fn dry_run_does_not_claim_saved_outputs() {
        let outputs = vec![
            "/tmp/preview/a.srt".into(),
            "/tmp/preview/a.vtt".into(),
            "/tmp/preview/b.srt".into(),
        ];
        let (title, detail) = completion_copy(1, &outputs, true);
        assert!(title.contains("Dry run complete"));
        assert!(title.contains("3 planned files"));
        assert!(detail.contains("SRT · VTT"));
        assert!(detail.contains("No files were written"));
        assert!(!title.contains("ready"));
        assert!(!detail.contains("Saved"));
    }

    #[test]
    fn empty_completion_does_not_claim_subtitles_ready() {
        let (title, detail) = completion_copy(0, &[], false);
        assert_eq!(title, "Finished without output files");
        assert!(detail.contains("No subtitle files were written"));
    }

    #[test]
    fn saved_completion_lists_unique_formats_and_location() {
        let outputs = vec![
            "/tmp/output/a.srt".into(),
            "/tmp/output/a.vtt".into(),
            "/tmp/output/b.srt".into(),
        ];
        let (title, detail) = completion_copy(1, &outputs, false);
        assert!(title.contains("3 files"));
        assert_eq!(detail, "Formats: SRT · VTT. Saved in /tmp/output");
    }
}

fn progress_card<'a>(
    progress: &'a crate::app::TranscribeProgress,
    input_path: &str,
) -> Element<'a, Message> {
    let header = column![
        text("Generating subtitles".to_string()).size(15),
        text(truncate_middle(input_path, 90))
            .size(11)
            .color(t::TEXT_FAINT),
    ]
    .spacing(2);

    let bar = progress_bar(0.0..=100.0, progress.progress as f32).height(Length::Fixed(8.0));

    let mut status_line = format!("{}%  ·  {}", progress.progress, progress.status_message);
    if let (Some(c), Some(t_)) = (progress.current_item, progress.total_items) {
        if t_ > 1 {
            status_line.push_str(&format!("  ·  File {c} of {t_}"));
        }
    }

    let mut cancel_btn = button(
        text(if progress.cancelling {
            "Cancelling…"
        } else {
            "Cancel"
        })
        .size(12),
    )
    .padding([6, 14])
    .style(t::secondary_button);
    if !progress.cancelling {
        cancel_btn = cancel_btn.on_press(Message::CancelTranscribe);
    }

    let footer = row![
        text(status_line).size(12).color(t::TEXT_MUTED),
        Space::with_width(Length::Fill),
        button(text("View activity".to_string()).size(12))
            .padding([6, 12])
            .on_press(Message::NavigateTo(Screen::Logs))
            .style(t::ghost_button),
        cancel_btn,
    ]
    .align_y(Alignment::Center)
    .spacing(8);

    container(column![header, bar, footer].spacing(10))
        .style(t::surface)
        .padding(18)
        .width(Length::Fill)
        .into()
}

fn truncate_middle(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let tail: String = s
        .chars()
        .rev()
        .take(n)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

fn section<'a>(title: &str, hint: &str, body: Element<'a, Message>) -> Element<'a, Message> {
    let header = column![
        text(title.to_string()).size(15),
        text(hint.to_string()).size(11).color(t::TEXT_MUTED),
    ]
    .spacing(2);

    container(column![header, body].spacing(12))
        .style(t::surface)
        .padding(18)
        .width(Length::Fill)
        .into()
}

fn chip<'a>(label: &str, active: bool) -> Element<'a, Message> {
    let color = if active { t::TEXT } else { t::WARN };
    container(text(label.to_string()).size(12).color(color))
        .style(t::surface_inset)
        .padding([6, 12])
        .into()
}

fn format_selector<'a>(s: &subtly_core::settings::Settings, busy: bool) -> Row<'a, Message> {
    let formats = [
        ("srt", "SRT"),
        ("vtt", "VTT"),
        ("txt", "Text"),
        ("json", "JSON"),
        ("csv", "CSV"),
    ];
    let mut row_widget: Row<'_, Message> = Row::new().spacing(6);
    for (id, label) in formats {
        let selected = s.export_formats.iter().any(|f| f == id);
        let prefix = if selected { "✓ " } else { "  " };
        let color = if selected { t::TEXT } else { t::TEXT_MUTED };
        let mut btn = button(text(format!("{prefix}{label}")).size(12).color(color))
            .padding([6, 12])
            .style(if selected {
                t::secondary_button
            } else {
                t::ghost_button
            });
        if !busy {
            btn = btn.on_press(Message::ToggleExportFormat(id.to_string()));
        }
        row_widget = row_widget.push(btn);
    }
    row_widget
}
