use leptos::prelude::*;

use super::icons::IconArrowUpRight;

#[component]
pub fn ErrorDisplay(#[prop(into)] error: String) -> impl IntoView {
    if error.contains("/pricing") {
        // Extract the feature description from the error message
        // Pattern: "Feature X requires a Y plan. Upgrade at /pricing"
        let description = error
            .split(". Upgrade")
            .next()
            .unwrap_or(&error)
            .to_string();

        view! {
            <div class="upgrade-banner">
                <div class="upgrade-banner-icon">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M12 2L2 7l10 5 10-5-10-5Z" />
                        <path d="M2 17l10 5 10-5" />
                        <path d="M2 12l10 5 10-5" />
                    </svg>
                </div>
                <div class="upgrade-banner-text">
                    <div class="upgrade-banner-title">{description}</div>
                    <div class="upgrade-banner-desc">
                        "Unlock this feature by upgrading your plan."
                    </div>
                </div>
                <a href="/pricing" class="btn btn-primary btn-sm">
                    "View plans" <IconArrowUpRight />
                </a>
            </div>
        }.into_any()
    } else {
        view! {
            <div class="flash flash-error">{error}</div>
        }
        .into_any()
    }
}
