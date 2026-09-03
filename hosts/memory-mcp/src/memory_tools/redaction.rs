use std::sync::OnceLock;

use regex::Regex;

const MAX_LENGTH: usize = 512;

fn url_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"[a-zA-Z][a-zA-Z0-9+\-.]*://\S+").expect("url redaction pattern is valid")
    })
}

fn windows_path_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:[A-Za-z]:\\|\\\\)\S+")
            .expect("windows path redaction pattern is valid")
    })
}

fn unix_path_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"/\S+").expect("unix path redaction pattern is valid"))
}

fn credential_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"sk-[A-Za-z0-9]+").expect("api key redaction pattern is valid"),
            Regex::new(r"Bearer\s+\S+").expect("bearer token redaction pattern is valid"),
            Regex::new(r"(?i)(?:api[_-]?key|token|secret|password)\s*[=:]\s*\S+")
                .expect("credential kv redaction pattern is valid"),
        ]
    })
}

pub(crate) fn redact(input: &str) -> String {
    let mut result = input.to_string();

    result = url_pattern().replace_all(&result, "<url>").to_string();
    result = windows_path_pattern()
        .replace_all(&result, "<path>")
        .to_string();
    result = unix_path_pattern()
        .replace_all(&result, "<path>")
        .to_string();
    for pattern in credential_patterns() {
        result = pattern.replace_all(&result, "<redacted>").to_string();
    }

    result.truncate(MAX_LENGTH);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_replaced() {
        let input = "see https://example.com/path?q=1 for details";
        assert_eq!(redact(input), "see <url> for details");
    }

    #[test]
    fn windows_path_replaced() {
        let input = "file at C:\\Users\\admin\\secret.db please";
        assert_eq!(redact(input), "file at <path> please");
    }

    #[test]
    fn unc_path_replaced() {
        let input = "access \\\\server\\share\\file.txt now";
        assert_eq!(redact(input), "access <path> now");
    }

    #[test]
    fn unix_path_replaced() {
        let input = "stored at /var/data/memory.db okay";
        assert_eq!(redact(input), "stored at <path> okay");
    }

    #[test]
    fn api_key_replaced() {
        let input = "using sk-abc123def456 for auth";
        assert_eq!(redact(input), "using <redacted> for auth");
    }

    #[test]
    fn bearer_token_replaced() {
        let input = "Authorization: Bearer tok_123abc";
        assert_eq!(redact(input), "Authorization: <redacted>");
    }

    #[test]
    fn key_equals_replaced() {
        let input = "api_key=sk-secret123 in config";
        assert_eq!(redact(input), "<redacted> in config");
    }

    #[test]
    fn plain_message_unchanged() {
        let input = "memory storage operation failed";
        assert_eq!(redact(input), input);
    }

    #[test]
    fn long_message_truncated() {
        let input = "x".repeat(1000);
        assert_eq!(redact(&input).len(), MAX_LENGTH);
    }
}
