use leptos::prelude::*;

use crate::components::error_display::ErrorDisplay;

#[server]
async fn create_org(slug: String, display_name: String) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, set_session_org};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_control_pool().await?;

    let org = db::create_organization(&pool, &slug, &display_name, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    db::add_org_member(&pool, org.id, user.id, "owner")
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Switch to the newly created org
    set_session_org(&user, &slug).await;

    leptos_axum::redirect(&format!("/orgs/{}/settings", slug));
    Ok(())
}

#[component]
pub fn OrgNewPage() -> impl IntoView {
    let create_action = ServerAction::<CreateOrg>::new();
    let error = move || {
        create_action.value().get().and_then(|r| {
            r.err().map(|e| e.to_string())
        })
    };

    view! {
        <div class="auth-container animate-in">
            <div class="card">
                <h1 class="card-header">"Create Organization"</h1>
                {move || error().map(|e| view! {
                    <ErrorDisplay error=e />
                })}
                <ActionForm action=create_action>
                    <div class="form-group">
                        <label for="slug">"Slug"</label>
                        <input type="text" id="slug" name="slug" required
                            placeholder="my-org"
                            pattern="[a-zA-Z0-9][a-zA-Z0-9_-]*"
                            title="Letters, numbers, hyphens, and underscores" />
                        <small class="form-hint">"URL-safe identifier (e.g. my-org)"</small>
                    </div>
                    <div class="form-group">
                        <label for="display_name">"Display Name"</label>
                        <input type="text" id="display_name" name="display_name" required
                            placeholder="My Organization" />
                    </div>
                    <button type="submit" class="btn btn-primary btn-full">
                        "Create Organization"
                    </button>
                </ActionForm>
            </div>
        </div>
    }
}
