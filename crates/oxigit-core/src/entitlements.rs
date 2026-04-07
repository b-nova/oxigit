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
        "pro" | "founding" => PlanEntitlements {
            max_private_repos: None,
            ai_features: true,
            deploy_previews: true,
            team_features: false,
        },
        "team" => PlanEntitlements {
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
