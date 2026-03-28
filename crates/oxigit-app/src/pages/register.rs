use leptos::prelude::*;

#[server]
async fn register_user(
    username: String,
    email: String,
    password: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_pool, set_session_user};
    use oxigit_core::db;

    let pool = get_pool().await?;
    let user = db::create_user(&pool, &username, &email, &password)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    set_session_user(user.id, &user.username).await;
    leptos_axum::redirect("/repos");
    Ok(())
}

#[component]
pub fn RegisterPage() -> impl IntoView {
    let register_action = ServerAction::<RegisterUser>::new();
    let error = move || {
        register_action.value().get().and_then(|r| {
            r.err().map(|e| e.to_string())
        })
    };

    view! {
        <div class="auth-container">
            <div class="card">
                <h1 class="card-header">"Create your account"</h1>
                {move || error().map(|e| view! {
                    <div class="flash flash-error">{e}</div>
                })}
                <ActionForm action=register_action>
                    <div class="form-group">
                        <label for="username">"Username"</label>
                        <input type="text" id="username" name="username" required autocomplete="username" />
                    </div>
                    <div class="form-group">
                        <label for="email">"Email"</label>
                        <input type="email" id="email" name="email" required autocomplete="email" />
                    </div>
                    <div class="form-group">
                        <label for="password">"Password"</label>
                        <input type="password" id="password" name="password" required minlength="8" autocomplete="new-password" />
                    </div>
                    <button type="submit" class="btn btn-primary" style="width: 100%;">
                        "Create account"
                    </button>
                </ActionForm>
                <p style="text-align: center; margin-top: 1rem; font-size: 0.875rem; color: var(--text-secondary);">
                    "Already have an account? " <a href="/login">"Sign in"</a>
                </p>
            </div>
        </div>
    }
}
