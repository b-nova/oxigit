use leptos::prelude::*;

/// Full-page loading skeleton with shimmer animation.
/// Use as Suspense fallback for page-level data fetches.
#[component]
pub fn LoadingPage() -> impl IntoView {
    view! {
        <div class="skeleton-page">
            <div class="skeleton-line skeleton-line-lg"></div>
            <div class="skeleton-line skeleton-line-md"></div>
            <div class="skeleton-line skeleton-line-sm"></div>
            <div class="skeleton-spacer"></div>
            <div class="skeleton-line skeleton-line-full"></div>
            <div class="skeleton-line skeleton-line-full"></div>
            <div class="skeleton-line skeleton-line-md"></div>
        </div>
    }
}

/// Card-level loading skeleton.
/// Use as Suspense fallback inside card containers or list views.
#[component]
pub fn LoadingCard() -> impl IntoView {
    view! {
        <div class="skeleton-card">
            <div class="skeleton-line skeleton-line-md"></div>
            <div class="skeleton-line skeleton-line-sm"></div>
        </div>
    }
}

/// Small inline spinner.
/// Use for inline data loading (selects, small sections).
#[component]
pub fn LoadingInline() -> impl IntoView {
    view! {
        <span class="loading-spinner"></span>
    }
}
