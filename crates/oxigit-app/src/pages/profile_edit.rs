use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub username: String,
    pub display_name: String,
    pub email: String,
}

#[server]
async fn fetch_profile() -> Result<ProfileInfo, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_control_pool().await?;
    let user = db::get_user_by_id(&pool, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(ProfileInfo {
        username: user.username,
        display_name: user.display_name,
        email: user.email,
    })
}

#[server]
async fn save_profile(display_name: String, email: String) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let email = email.trim().to_string();
    if email.is_empty() || !email.contains('@') {
        return Err(ServerFnError::new("Please enter a valid email address"));
    }

    let display_name = display_name.trim().to_string();

    let pool = get_control_pool().await?;
    db::update_user_profile(&pool, user.id, &display_name, &email)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") && msg.contains("email") {
                ServerFnError::new("This email address is already taken")
            } else {
                ServerFnError::new(msg)
            }
        })?;

    Ok(())
}

#[component]
pub fn ProfileEditPage() -> impl IntoView {
    let profile = Resource::new(|| (), |_| fetch_profile());
    let save_action = ServerAction::<SaveProfile>::new();

    let save_success = move || {
        save_action
            .value()
            .get()
            .and_then(|r| r.ok())
            .map(|_| true)
    };
    let save_error = move || {
        save_action
            .value()
            .get()
            .and_then(|r| r.err().map(|e| e.to_string()))
    };

    // Refetch profile after save
    Effect::new(move || {
        save_action.version().get();
        profile.refetch();
    });

    view! {
        <div class="page-header">
            <h1 class="page-title">"Profile"</h1>
        </div>

        <div class="card" style="max-width: 600px;">
            <div class="card-header">"Edit Profile"</div>
            {move || save_success().map(|_| view! {
                <div class="flash flash-success">"Profile updated."</div>
            })}
            {move || save_error().map(|e| view! {
                <ErrorDisplay error=e />
            })}
            <Suspense fallback=|| view! { <LoadingPage /> }>
                {move || Suspend::new(async move {
                    match profile.await {
                        Ok(info) => {
                            let default_display_name = info.display_name.clone();
                            let default_email = info.email.clone();
                            view! {
                                <ActionForm action=save_action>
                                    <div class="form-group">
                                        <label>"Username"</label>
                                        <input
                                            type="text"
                                            value=info.username
                                            disabled
                                            class="form-input-disabled"
                                        />
                                        <p class="text-secondary" style="font-size: 0.75rem; margin-top: var(--space-1);">
                                            "Username cannot be changed."
                                        </p>
                                    </div>
                                    <div class="form-group">
                                        <label for="display_name">"Display name"</label>
                                        <input
                                            type="text"
                                            id="display_name"
                                            name="display_name"
                                            value=default_display_name
                                            placeholder="Your display name"
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label for="email">"Email"</label>
                                        <input
                                            type="email"
                                            id="email"
                                            name="email"
                                            value=default_email
                                            required
                                            placeholder="your@email.com"
                                        />
                                    </div>
                                    <button type="submit" class="btn btn-primary">"Save"</button>
                                </ActionForm>
                            }.into_any()
                        }
                        Err(e) => view! {
                            <ErrorDisplay error=e.to_string() />
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
