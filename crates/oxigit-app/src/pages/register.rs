use leptos::prelude::*;

use super::get_current_user;

#[server]
async fn register_user(
    username: String,
    email: String,
    password: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_control_pool, set_session_user};
    use oxigit_core::db;

    let pool = get_control_pool().await?;

    // First registered user becomes admin automatically
    let is_first_user = db::count_users(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))? == 0;

    let user = db::create_user(&pool, &username, &email, &password)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if is_first_user {
        db::set_user_admin(&pool, user.id, true)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    // Create personal org for the new user
    let org = db::create_organization(&pool, &username, &username, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    db::add_org_member(&pool, org.id, user.id, "owner")
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    set_session_user(user.id, &user.username, Some(&username)).await;
    leptos_axum::redirect("/repos");
    Ok(())
}

#[component]
pub fn RegisterPage() -> impl IntoView {
    let user = Resource::new(|| (), |_| get_current_user());
    let register_action = ServerAction::<RegisterUser>::new();
    let error = move || {
        register_action.value().get().and_then(|r| {
            r.err().map(|e| e.to_string())
        })
    };

    view! {
        <Suspense fallback=|| ()>
            {move || Suspend::new(async move {
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
                                <button type="submit" class="btn btn-primary btn-full">
                                    "Create account"
                                </button>
                            </ActionForm>
                            <p class="auth-footer">
                                "Already have an account? " <a href="/login">"Sign in"</a>
                            </p>
                        </div>
                    </div>
                }.into_any()
            })}
        </Suspense>
    }
}
