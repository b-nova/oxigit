use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="auth-container" style="text-align: center; margin-top: 5rem;">
            <h1 style="font-size: 2.5rem; margin-bottom: 0.5rem;">"Oxigit"</h1>
            <p style="color: var(--text-secondary); margin-bottom: 2rem;">
                "A self-hosted Git platform, built with Rust."
            </p>
            <div style="display: flex; gap: 1rem; justify-content: center;">
                <a href="/login" class="btn">"Sign in"</a>
                <a href="/register" class="btn btn-primary">"Get started"</a>
            </div>
        </div>
    }
}
