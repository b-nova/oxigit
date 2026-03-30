use leptos::prelude::*;

use super::get_current_user;

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
    let user = Resource::new(|| (), |_| get_current_user());
    let login_action = ServerAction::<LoginUser>::new();
    let error = move || {
        login_action.value().get().and_then(|r| {
            r.err().map(|e| e.to_string())
        })
    };

    view! {
        <Suspense fallback=|| ()>
            {move || Suspend::new(async move {
                // Redirect if already logged in
                if let Ok(Some(_)) = user.await {
                    return view! {
                        <div class="auth-container text-center">
                            <p>"You are already signed in."</p>
                            <a href="/repos" class="btn btn-primary mt-4">"Go to Repositories"</a>
                        </div>
                    }.into_any();
                }

                view! {
                    <div class="auth-container animate-in">
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
                                <button type="submit" class="btn btn-primary btn-full">
                                    "Sign in"
                                </button>
                            </ActionForm>
                            <p class="auth-footer">
                                "Don't have an account? " <a href="/register">"Sign up"</a>
                            </p>
                        </div>
                    </div>
                }.into_any()
            })}
        </Suspense>
    }
}
