pub fn format_bytes(bytes: u64) -> String {
    const K: f64 = 1024.0;
    if bytes == 0 {
        return "0 B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let i = (bytes as f64).log(K).floor() as usize;
    let i = i.min(units.len() - 1);
    let value = bytes as f64 / K.powi(i as i32);
    format!("{value:.1} {}", units[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }
}
