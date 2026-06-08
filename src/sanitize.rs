//! Intellectual Property Guard & Data Stripping for secure indexing.

use regex::Regex;
use std::sync::OnceLock;

static HIGH_RISK_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

fn get_patterns() -> &'static Vec<(Regex, &'static str)> {
    HIGH_RISK_PATTERNS.get_or_init(|| {
        vec![
            // Private Keys (RSA, EC, Generic private block formats)
            (
                Regex::new(r"(?i)-----BEGIN [A-Z ]+ PRIVATE KEY-----[\s\S]+?-----END [A-Z ]+ PRIVATE KEY-----").unwrap(),
                "[PRIVATE_KEY_REDACTED]"
            ),
            // Cloud Credentials / API Keys assignment patterns
            (
                Regex::new(r#"(?i)(api[_-]?key|auth[_-]?token|secret|password|passwd|private[_-]?key|db[_-]?pass)\s*[:=]\s*['"]([^'"]{4,})['"]"#).unwrap(),
                "$1 = \"[SECRET_REDACTED]\""
            ),
            // AWS Credentials
            (
                Regex::new(r#"(?i)aws[_-]?(secret|access)[_-]?(key|id)?\s*[:=]\s*['"]([^'"]{4,})['"]"#).unwrap(),
                "aws_secret = \"[AWS_SECRET_REDACTED]\""
            ),
            // Database Connection URI credentials
            (
                Regex::new(r"(?i)(mongodb\+srv|postgres|postgresql|mysql|mssql)://[^:]+:([^@]+)@").unwrap(),
                "$1://[USER]:[PASSWORD_REDACTED]@"
            ),
        ]
    })
}

/// Sanitize content block by scrubbing hardcoded environment variables,
/// keys, credentials, and database connection URIs.
pub fn sanitize_content(content: &str) -> String {
    let mut sanitized = content.to_string();
    for (re, replacement) in get_patterns() {
        sanitized = re.replace_all(&sanitized, *replacement).to_string();
    }
    sanitized
}
