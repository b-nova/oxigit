/// AI access level — distinguishes free-tier preview from full paid access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAccessLevel {
    /// Free plan: limited views (badges, 30-day metrics, 10 prompts, view-only hub)
    Limited,
    /// Pro/Team/Founding: full access to all AI features
    Full,
}

pub fn ai_access_for_plan(plan: &str) -> AiAccessLevel {
    match plan {
        "flat" | "founding" | "team" | "enterprise" => AiAccessLevel::Full,
        _ => AiAccessLevel::Limited,
    }
}

/// Plan entitlements — defines what each subscription tier allows.

#[derive(Debug, Clone)]
pub struct PlanEntitlements {
    /// Maximum number of private repos. `None` means unlimited.
    pub max_private_repos: Option<usize>,
    pub ai_features: bool,
    pub deploy_previews: bool,
    pub team_features: bool,
}

pub fn for_plan(plan: &str) -> PlanEntitlements {
    match plan {
        "flat" | "founding" => PlanEntitlements {
            max_private_repos: None,
            ai_features: true,
            deploy_previews: true,
            team_features: false,
        },
        "team" | "enterprise" => PlanEntitlements {
            max_private_repos: None,
            ai_features: true,
            deploy_previews: true,
            team_features: true,
        },
        _ => PlanEntitlements {
            max_private_repos: Some(5),
            ai_features: false,
            deploy_previews: false,
            team_features: false,
        },
    }
}
