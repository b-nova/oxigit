use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[server]
async fn login_user(username: String, password: String) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_pool, set_session_user};
    use oxigit_core::db;

    let pool = get_pool().await?;
    let user = db::authenticate_user(&pool, &username, &password)
        .await
        .map_err(|_| ServerFnError::new("Invalid username or password"))?;

    set_session_user(user.id, &user.username).await;
    leptos_axum::redirect("/repos");
    Ok(())
}

#[component]
pub fn LoginPage() -> impl IntoView {
    let login_action = ServerAction::<LoginUser>::new();
    let error = move || {
        login_action.value().get().and_then(|r| {
            r.err().map(|e| e.to_string())
        })
    };

    view! {
        <div class="auth-container">
            <div class="card">
                <h1 class="card-header">"Sign in to Oxigit"</h1>
                {move || error().map(|e| view! {
                    <div class="flash flash-error">{e}</div>
                })}
                <ActionForm action=login_action>
                    <div class="form-group">
                        <label for="username">"Username"</label>
                        <input type="text" id="username" name="username" required autocomplete="username" />
                    </div>
                    <div class="form-group">
                        <label for="password">"Password"</label>
                        <input type="password" id="password" name="password" required autocomplete="current-password" />
                    </div>
                    <button type="submit" class="btn btn-primary" style="width: 100%;">
                        "Sign in"
                    </button>
                </ActionForm>
                <p style="text-align: center; margin-top: 1rem; font-size: 0.875rem; color: var(--text-secondary);">
                    "Don't have an account? " <a href="/register">"Sign up"</a>
                </p>
            </div>
        </div>
    }
}
