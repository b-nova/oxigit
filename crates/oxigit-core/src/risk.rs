use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskCategory {
    Security,
    Breaking,
    Performance,
    Quality,
}

impl RiskCategory {
    pub fn label(&self) -> &str {
        match self {
            Self::Security => "security",
            Self::Breaking => "breaking",
            Self::Performance => "performance",
            Self::Quality => "quality",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFlag {
    pub category: RiskCategory,
    pub message: String,
    pub file: Option<String>,
}

/// Scan a unified diff for risk patterns. Returns a list of flagged issues.
pub fn scan_diff(diff: &str) -> Vec<RiskFlag> {
    let mut flags = Vec::new();
    let mut current_file: Option<String> = None;

    static RE_API_KEY: OnceLock<Regex> = OnceLock::new();
    static RE_HARDCODED_SECRET: OnceLock<Regex> = OnceLock::new();
    static RE_SQL_INJECT: OnceLock<Regex> = OnceLock::new();
    static RE_EVAL: OnceLock<Regex> = OnceLock::new();
    static RE_UNSAFE: OnceLock<Regex> = OnceLock::new();
    static RE_TODO: OnceLock<Regex> = OnceLock::new();
    static RE_PUB_REMOVED: OnceLock<Regex> = OnceLock::new();
    static RE_EXPORT_REMOVED: OnceLock<Regex> = OnceLock::new();

    let re_api_key = RE_API_KEY.get_or_init(|| {
        Regex::new(
            r#"(?i)(api[_-]?key|secret[_-]?key|auth[_-]?token|password)\s*[:=]\s*["'][^"']{8,}"#,
        )
        .expect("valid regex")
    });
    let re_hardcoded_secret = RE_HARDCODED_SECRET.get_or_init(|| {
        Regex::new(r"(AKIA[0-9A-Z]{16}|ghp_[a-zA-Z0-9]{36}|sk-[a-zA-Z0-9]{32,}|-----BEGIN (RSA |EC )?PRIVATE KEY)").expect("valid regex")
    });
    let re_sql_inject = RE_SQL_INJECT.get_or_init(|| {
        Regex::new(r#"(?i)format!\s*\(\s*"[^"]*(?:SELECT|INSERT|UPDATE|DELETE|DROP)[^"]*\{"#)
            .expect("valid regex")
    });
    let re_eval = RE_EVAL.get_or_init(|| Regex::new(r"(?i)\b(eval|exec)\s*\(").expect("valid regex"));
    let re_unsafe = RE_UNSAFE.get_or_init(|| Regex::new(r"\bunsafe\s*\{").expect("valid regex"));
    let re_todo = RE_TODO.get_or_init(|| Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX|WORKAROUND)\b").expect("valid regex"));
    let re_pub_removed = RE_PUB_REMOVED.get_or_init(|| {
        Regex::new(r"^pub\s+(fn|struct|enum|trait|type|const|static)\s+\w+").expect("valid regex")
    });
    let re_export_removed = RE_EXPORT_REMOVED.get_or_init(|| {
        Regex::new(r"^export\s+(function|const|let|class|default|type|interface)\s+").expect("valid regex")
    });

    for line in diff.lines() {
        // Track current file
        if line.starts_with("diff --git") {
            // Extract file name from "diff --git a/path b/path"
            if let Some(b_part) = line.split(" b/").last() {
                current_file = Some(b_part.to_string());
            }
            continue;
        }

        // Skip metadata lines
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            continue;
        }

        let file = current_file.clone();

        // Added lines: check security, quality, performance
        if let Some(content) = line.strip_prefix('+') {
            // Security: API keys / secrets
            if re_api_key.is_match(content) {
                flags.push(RiskFlag {
                    category: RiskCategory::Security,
                    message: "Possible hardcoded secret or API key".into(),
                    file: file.clone(),
                });
            }
            if re_hardcoded_secret.is_match(content) {
                flags.push(RiskFlag {
                    category: RiskCategory::Security,
                    message:
                        "Detected known secret pattern (AWS key, GitHub token, or private key)"
                            .into(),
                    file: file.clone(),
                });
            }

            // Security: SQL injection
            if re_sql_inject.is_match(content) {
                flags.push(RiskFlag {
                    category: RiskCategory::Security,
                    message: "Possible SQL injection via string formatting".into(),
                    file: file.clone(),
                });
            }

            // Security: eval/exec
            if re_eval.is_match(content) {
                flags.push(RiskFlag {
                    category: RiskCategory::Security,
                    message: "Use of eval() or exec() — potential code injection".into(),
                    file: file.clone(),
                });
            }

            // Security: unsafe blocks (Rust)
            if re_unsafe.is_match(content) {
                flags.push(RiskFlag {
                    category: RiskCategory::Security,
                    message: "Unsafe block added".into(),
                    file: file.clone(),
                });
            }

            // Quality: TODO/FIXME markers
            if re_todo.is_match(content) {
                flags.push(RiskFlag {
                    category: RiskCategory::Quality,
                    message: "TODO/FIXME/HACK comment added".into(),
                    file: file.clone(),
                });
            }
        }

        // Removed lines: check for breaking changes
        if let Some(content) = line.strip_prefix('-') {
            if re_pub_removed.is_match(content.trim()) {
                flags.push(RiskFlag {
                    category: RiskCategory::Breaking,
                    message: format!("Removed public API: {}", content.trim()),
                    file: file.clone(),
                });
            }
            if re_export_removed.is_match(content.trim()) {
                flags.push(RiskFlag {
                    category: RiskCategory::Breaking,
                    message: format!("Removed export: {}", content.trim()),
                    file: file.clone(),
                });
            }
        }
    }

    // Performance: check for binary file additions
    if diff.contains("Binary file") && diff.contains("differ") {
        flags.push(RiskFlag {
            category: RiskCategory::Performance,
            message: "Binary file added to repository".into(),
            file: None,
        });
    }

    // Deduplicate by (category, message, file)
    flags.dedup_by(|a, b| a.category == b.category && a.message == b.message && a.file == b.file);

    flags
}
