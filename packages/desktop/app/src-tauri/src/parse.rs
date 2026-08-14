//! Parsing the backend's stdout handshake.

/// The exact prefix the web-app bundle prints on its URL readiness line.
pub const URL_PREFIX: &str = "dsh web: ";

/// Extract the loopback URL from one backend stdout line, if it is the
/// readiness line. The line may carry a ` (LAN: ...)` suffix and a trailing
/// line ending; only `http://` URLs qualify (the harness never serves https).
pub fn parse_url_line(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix(URL_PREFIX)?;
    let url = rest.trim().split_whitespace().next()?;
    if url.starts_with("http://") {
        Some(url.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse_url_line;

    #[test]
    fn plain_url_line() {
        assert_eq!(
            parse_url_line("dsh web: http://127.0.0.1:3080"),
            Some("http://127.0.0.1:3080".to_string())
        );
    }

    #[test]
    fn url_line_with_lan_suffix() {
        assert_eq!(
            parse_url_line("dsh web: http://127.0.0.1:3080 (LAN: http://192.168.1.2:3080)"),
            Some("http://127.0.0.1:3080".to_string())
        );
    }

    #[test]
    fn url_line_with_crlf() {
        assert_eq!(
            parse_url_line("dsh web: http://127.0.0.1:3080\r\n"),
            Some("http://127.0.0.1:3080".to_string())
        );
    }

    #[test]
    fn surrounded_by_whitespace() {
        assert_eq!(
            parse_url_line("  dsh web: http://127.0.0.1:3080  "),
            Some("http://127.0.0.1:3080".to_string())
        );
    }

    #[test]
    fn prefix_without_space_is_not_a_match() {
        assert_eq!(parse_url_line("dsh web:http://127.0.0.1:3080"), None);
    }

    #[test]
    fn non_http_url_is_rejected() {
        assert_eq!(parse_url_line("dsh web: https://127.0.0.1:3080"), None);
    }

    #[test]
    fn unrelated_lines_are_ignored() {
        assert_eq!(parse_url_line("info: booting"), None);
        assert_eq!(parse_url_line(""), None);
    }
}
