//! Bounded, read-only excerpts from files returned by a successful real run.
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub timecode: String,
    pub text: String,
}
#[derive(Debug, Clone)]
pub struct Preview {
    pub path: String,
    pub cues: Vec<Cue>,
}

pub fn read(outputs: &[String], dry_run: bool) -> Option<Preview> {
    if dry_run {
        return None;
    }
    for path in outputs {
        let Some(extension) = std::path::Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
        else {
            continue;
        };
        if !extension.eq_ignore_ascii_case("srt") && !extension.eq_ignore_ascii_case("vtt") {
            continue;
        }
        let mut bytes = Vec::new();
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        if file.take(65_536).read_to_end(&mut bytes).is_err() {
            continue;
        }
        let content = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(error) if error.error_len().is_none() => {
                match std::str::from_utf8(&bytes[..error.valid_up_to()]) {
                    Ok(text) => text,
                    Err(_) => continue,
                }
            }
            Err(_) => continue,
        };
        let cues = parse(content);
        if !cues.is_empty() {
            return Some(Preview {
                path: path.clone(),
                cues,
            });
        }
    }
    None
}
fn parse(content: &str) -> Vec<Cue> {
    let normalized = content.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .filter_map(|block| {
            let mut lines = block.lines();
            let timecode = lines.find(|line| line.contains(" --> "))?;
            let text = lines.take(4).collect::<Vec<_>>().join("\n");
            if text.trim().is_empty() {
                return None;
            }
            Some(Cue {
                timecode: timecode.chars().take(80).collect(),
                text: text.chars().take(300).collect(),
            })
        })
        .take(3)
        .collect()
}
pub fn timecode(ms: i64) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        ms % 1000
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_real_written_cues_and_never_reads_dry_run() {
        let path =
            std::env::temp_dir().join(format!("subtly-preview-{}.srt", uuid::Uuid::new_v4()));
        std::fs::write(&path,"1\n00:00:01,000 --> 00:00:03,250\nA real saved line.\n\n2\n00:00:03,250 --> 00:00:04,000\nNext line.\n").unwrap();
        let outputs = vec![path.display().to_string()];
        assert!(read(&outputs, true).is_none());
        let preview = read(&outputs, false).unwrap();
        assert_eq!(preview.cues.len(), 2);
        assert_eq!(preview.cues[0].text, "A real saved line.");
        assert_eq!(preview.cues[1].timecode, "00:00:03,250 --> 00:00:04,000");
        std::fs::remove_file(path).unwrap();
        assert!(read(&outputs, false).is_none());
    }
    #[test]
    fn vtt_crlf_is_bounded_and_ignores_noncue_headers() {
        let cues=parse("WEBVTT\r\n\r\nNOTE metadata\r\n\r\n00:01.000 --> 00:02.000\r\nFirst caption.\r\n\r\n00:02.000 --> 00:04.000\r\nSecond caption.\r\n");
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "First caption.");
    }
}
