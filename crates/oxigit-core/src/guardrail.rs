/// AI Guardrail evaluation — checks diffs against configured rules.
use crate::models::{GuardrailConfig, GuardrailRule};
use crate::risk;

/// A guardrail violation produced by evaluation.
#[derive(Debug, Clone)]
pub struct Violation {
    pub category: String,
    pub action: String,   // "block" or "warn"
    pub severity: String, // "low", "medium", "high", "critical"
    pub message: String,
    pub file_path: Option<String>,
}

/// Evaluate a diff against guardrail rules.
/// Returns violations for rules that match (both block and warn).
pub fn evaluate_diff(
    rules: &[GuardrailRule],
    config: &Option<GuardrailConfig>,
    diff: &str,
    file_count: usize,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    // Build a map of category -> action for enabled rules
    let rule_map: std::collections::HashMap<&str, &str> = rules
        .iter()
        .filter(|r| r.action != "off")
        .map(|r| (r.category.as_str(), r.action.as_str()))
        .collect();

    // Scan diff for risk flags
    if !rule_map.is_empty() && !diff.is_empty() {
        let risk_flags = risk::scan_diff(diff);

        for flag in risk_flags {
            let category_label = flag.category.label();
            if let Some(&action) = rule_map.get(category_label) {
                let severity = match flag.category {
                    risk::RiskCategory::Security => "critical",
                    risk::RiskCategory::Breaking => "high",
                    risk::RiskCategory::Performance => "medium",
                    risk::RiskCategory::Quality => "low",
                };
                violations.push(Violation {
                    category: category_label.to_string(),
                    action: action.to_string(),
                    severity: severity.to_string(),
                    message: flag.message,
                    file_path: flag.file,
                });
            }
        }
    }

    // Check max files per push
    if let Some(cfg) = config
        && let Some(max_files) = cfg.max_files_per_push
        && file_count as i64 > max_files
    {
        // Use the strictest active action for this violation
        let action = if rule_map.values().any(|&a| a == "block") {
            "block"
        } else {
            "warn"
        };
        violations.push(Violation {
            category: "max_files".to_string(),
            action: action.to_string(),
            severity: "medium".to_string(),
            message: format!("Push touches {} files (limit: {})", file_count, max_files),
            file_path: None,
        });
    }

    violations
}

/// Check if any violations have the "block" action.
pub fn has_blocking_violations(violations: &[Violation]) -> bool {
    violations.iter().any(|v| v.action == "block")
}

/// Format violations into a human-readable message for git push rejection.
pub fn format_block_message(violations: &[Violation]) -> String {
    let mut msg = String::from("Push rejected by AI guardrails:\n");
    for v in violations.iter().filter(|v| v.action == "block") {
        if let Some(ref file) = v.file_path {
            msg.push_str(&format!("  [{}] {} ({})\n", v.category, v.message, file));
        } else {
            msg.push_str(&format!("  [{}] {}\n", v.category, v.message));
        }
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rules(security: &str, quality: &str) -> Vec<GuardrailRule> {
        vec![
            GuardrailRule {
                id: 1,
                repo_id: 1,
                category: "security".into(),
                action: security.into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
            GuardrailRule {
                id: 2,
                repo_id: 1,
                category: "quality".into(),
                action: quality.into(),
                created_at: String::new(),
                updated_at: String::new(),
            },
        ]
    }

    #[test]
    fn detects_security_violation() {
        let rules = make_rules("warn", "off");
        let diff = "diff --git a/config.rs b/config.rs\n--- a/config.rs\n+++ b/config.rs\n@@ -1 +1 @@\n+let api_key = \"sk-1234567890abcdef\";";
        let violations = evaluate_diff(&rules, &None, diff, 1);
        assert!(!violations.is_empty(), "Expected security violation");
        assert_eq!(violations[0].category, "security");
        assert_eq!(violations[0].action, "warn");
    }

    #[test]
    fn off_rules_produce_no_violations() {
        let rules = make_rules("off", "off");
        let diff = "diff --git a/x b/x\n+++ b/x\n+let password = \"secret123\";";
        let violations = evaluate_diff(&rules, &None, diff, 1);
        assert!(violations.is_empty());
    }

    #[test]
    fn max_files_check() {
        let rules = make_rules("warn", "off");
        let config = Some(GuardrailConfig {
            id: 1,
            repo_id: 1,
            min_vibe_score: None,
            max_files_per_push: Some(3),
            created_at: String::new(),
            updated_at: String::new(),
        });
        let violations = evaluate_diff(&rules, &config, "", 5);
        assert!(violations.iter().any(|v| v.category == "max_files"));
    }

    #[test]
    fn has_blocking() {
        let v = vec![Violation {
            category: "security".into(),
            action: "block".into(),
            severity: "critical".into(),
            message: "test".into(),
            file_path: None,
        }];
        assert!(has_blocking_violations(&v));

        let v2 = vec![Violation {
            category: "quality".into(),
            action: "warn".into(),
            severity: "low".into(),
            message: "test".into(),
            file_path: None,
        }];
        assert!(!has_blocking_violations(&v2));
    }
}
