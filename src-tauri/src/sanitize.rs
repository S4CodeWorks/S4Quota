use regex::Regex;
use std::sync::LazyLock;

const MAX_ERROR_CHARS: usize = 2_048;

static SECRET_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)((?:\"?(?:access[_-]?token|refresh[_-]?token|id[_-]?token|authorization|email|account[_-]?id|chatgpt[_-]?account[_-]?id)\"?)\s*[:=]\s*)(?:\"[^\"]*\"|'[^']*'|[^\s,}]+)"#,
    )
    .expect("valid secret regex")
});
static BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(bearer\s+)[^\s,;]+").expect("valid bearer regex"));
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").expect("valid email regex")
});
static API_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bsk-[A-Za-z0-9_-]{12,}\b").expect("valid key regex"));

pub fn sanitize_text(input: &str) -> String {
    let redacted = BEARER.replace_all(input, "$1[REDACTED]");
    let redacted = SECRET_VALUE.replace_all(&redacted, "$1[REDACTED]");
    let redacted = EMAIL.replace_all(&redacted, "[REDACTED_EMAIL]");
    let redacted = API_KEY.replace_all(&redacted, "[REDACTED_KEY]");
    redacted.chars().take(MAX_ERROR_CHARS).collect()
}

#[derive(Debug)]
pub struct BoundedDiagnostics {
    content: String,
    max_bytes: usize,
}

impl BoundedDiagnostics {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            content: String::new(),
            max_bytes,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        self.content.push_str(&sanitize_text(&text));
        if self.content.len() > self.max_bytes {
            let mut start = self.content.len() - self.max_bytes;
            while !self.content.is_char_boundary(start) {
                start += 1;
            }
            self.content.drain(..start);
        }
    }

    #[allow(dead_code)]
    pub fn snapshot(&self) -> String {
        self.content.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_credentials_identity_and_email() {
        let value = sanitize_text(
            r#"authorization=Bearer abcdef email=user@example.com access_token=secret sk-abcdefghijklmnopqrstuvwxyz"#,
        );
        assert!(!value.contains("abcdef"));
        assert!(!value.contains("user@example.com"));
        assert!(!value.contains("secret"));
        assert!(!value.contains("sk-abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn diagnostics_are_bounded_and_sanitized() {
        let mut buffer = BoundedDiagnostics::new(32);
        buffer.push(b"email=user@example.com\n");
        buffer.push(&vec![b'x'; 128]);
        let value = buffer.snapshot();
        assert!(value.len() <= 32);
        assert!(!value.contains("user@example.com"));
    }
}
