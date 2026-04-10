/// Vibe Score — AI coding session quality metrics.
///
/// Computes a 0–100 quality score for an AI coding session based on
/// efficiency, risk, churn, scope, and revert status.
pub struct SessionMetrics {
    pub commit_count: usize,
    pub prompt_count: usize,
    pub risk_flag_count: usize,
    pub files_touched: usize,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub was_reverted: bool,
}

/// Computed vibe score with breakdown.
pub struct VibeScore {
    pub score: u8,
    pub grade: char,
    pub factors: VibeFactors,
}

/// Individual factor scores (each 0.0–1.0).
pub struct VibeFactors {
    pub commits_per_prompt: f64,
    pub risk_density: f64,
    pub churn_ratio: f64,
    pub file_scope: f64,
    pub revert_penalty: f64,
}

/// Compute a vibe score from session metrics.
pub fn compute_vibe_score(m: &SessionMetrics) -> VibeScore {
    let prompt_count = m.prompt_count.max(1) as f64;
    let ratio = m.commit_count as f64 / prompt_count;

    // Factor 1: Commits per prompt (weight 25)
    // 1 commit/prompt = perfect, 5+ = worst
    let f_cpp = clamp(1.0 - (ratio - 1.0) / 4.0);

    // Factor 2: Risk density (weight 25)
    // 0 flags = perfect, 5+ = worst
    let f_risk = clamp(1.0 - m.risk_flag_count as f64 / 5.0);

    // Factor 3: Churn ratio (weight 20)
    // deletions / (additions + deletions); low = clean, high = back-and-forth
    let total_lines = (m.lines_added + m.lines_deleted).max(1) as f64;
    let churn = m.lines_deleted as f64 / total_lines;
    let f_churn = clamp(1.0 - churn / 0.8);

    // Factor 4: File scope (weight 15)
    // 1–5 files = perfect, 20+ = worst
    let f_files = if m.files_touched <= 5 {
        1.0
    } else {
        clamp(1.0 - (m.files_touched as f64 - 5.0) / 15.0)
    };

    // Factor 5: Revert penalty (weight 15)
    let f_revert = if m.was_reverted { 0.0 } else { 1.0 };

    let raw = f_cpp * 25.0 + f_risk * 25.0 + f_churn * 20.0 + f_files * 15.0 + f_revert * 15.0;
    let score = (raw.round() as u8).min(100);

    VibeScore {
        score,
        grade: grade_from_score(score),
        factors: VibeFactors {
            commits_per_prompt: f_cpp,
            risk_density: f_risk,
            churn_ratio: f_churn,
            file_scope: f_files,
            revert_penalty: f_revert,
        },
    }
}

/// Map a numeric score to a letter grade.
pub fn grade_from_score(score: u8) -> char {
    match score {
        80..=100 => 'A',
        60..=79 => 'B',
        40..=59 => 'C',
        20..=39 => 'D',
        _ => 'F',
    }
}

fn clamp(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_session() {
        let m = SessionMetrics {
            commit_count: 1,
            prompt_count: 1,
            risk_flag_count: 0,
            files_touched: 2,
            lines_added: 50,
            lines_deleted: 0,
            was_reverted: false,
        };
        let s = compute_vibe_score(&m);
        assert_eq!(s.score, 100);
        assert_eq!(s.grade, 'A');
    }

    #[test]
    fn terrible_session() {
        let m = SessionMetrics {
            commit_count: 10,
            prompt_count: 1,
            risk_flag_count: 10,
            files_touched: 30,
            lines_added: 10,
            lines_deleted: 90,
            was_reverted: true,
        };
        let s = compute_vibe_score(&m);
        assert!(s.score < 20, "Expected F grade, got score={}", s.score);
        assert_eq!(s.grade, 'F');
    }

    #[test]
    fn average_session() {
        let m = SessionMetrics {
            commit_count: 3,
            prompt_count: 2,
            risk_flag_count: 2,
            files_touched: 8,
            lines_added: 100,
            lines_deleted: 30,
            was_reverted: false,
        };
        let s = compute_vibe_score(&m);
        assert!(
            s.score >= 40 && s.score <= 80,
            "Expected B/C grade, got score={}",
            s.score
        );
    }

    #[test]
    fn grade_boundaries() {
        assert_eq!(grade_from_score(100), 'A');
        assert_eq!(grade_from_score(80), 'A');
        assert_eq!(grade_from_score(79), 'B');
        assert_eq!(grade_from_score(60), 'B');
        assert_eq!(grade_from_score(59), 'C');
        assert_eq!(grade_from_score(40), 'C');
        assert_eq!(grade_from_score(39), 'D');
        assert_eq!(grade_from_score(20), 'D');
        assert_eq!(grade_from_score(19), 'F');
        assert_eq!(grade_from_score(0), 'F');
    }
}
