use leptos::prelude::*;

use crate::components::error_display::ErrorDisplay;

#[server]
async fn create_repo(
    name: String,
    description: String,
    is_private: bool,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool, get_user_entitlements};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    if is_private {
        let entitlements = get_user_entitlements(user.id).await?;
        if let Some(max) = entitlements.max_private_repos {
            let count = {
                #[cfg(feature = "saas")]
                {
                    let multi = crate::server_fns::is_multi_tenant().await?;
                    if multi {
                        db::count_private_repos_from_index(&pool, user.id).await
                    } else {
                        db::count_private_repositories(&pool, user.id).await
                    }
                }
                #[cfg(not(feature = "saas"))]
                { db::count_private_repositories(&pool, user.id).await }
            }
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            if count as usize >= max {
                return Err(ServerFnError::new(format!(
                    "Free plan allows up to {} private repositories. Upgrade to Flat for unlimited.",
                    max
                )));
            }
        }
    }

    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{extract_active_org, get_tenant_mgr, is_multi_tenant};
        let org_slug = extract_active_org().await.unwrap_or_else(|| user.username.clone());

        if is_multi_tenant().await? {
            let tenant_mgr = get_tenant_mgr().await?;
            let tenant_pool = tenant_mgr.get_tenant_pool(&org_slug)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            let repos_dir = tenant_mgr.tenant_repos_dir(&org_slug);
            db::create_repository_in_tenant(
                &tenant_pool, user.id, &user.username, &name, &description, is_private, &repos_dir,
            )
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        } else {
            db::create_repository(&pool, user.id, &name, &description, is_private, &data_dir)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }

        // Register in the global repository index
        db::register_repo_in_index(&pool, &org_slug, user.id, &user.username, &name, &description, is_private)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    #[cfg(not(feature = "saas"))]
    {
        db::create_repository(&pool, user.id, &name, &description, is_private, &data_dir)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    leptos_axum::redirect(&format!("/{}/{}", user.username, name));
    Ok(())
}

#[component]
pub fn NewRepoPage() -> impl IntoView {
    let create_action = ServerAction::<CreateRepo>::new();
    let error = move || {
        create_action.value().get().and_then(|r| {
            r.err().map(|e| e.to_string())
        })
    };

    view! {
        <div class="auth-container animate-in">
            <div class="card">
                <h1 class="card-header">"Create a new repository"</h1>
                {move || error().map(|e| view! {
                    <ErrorDisplay error=e />
                })}
                <ActionForm action=create_action>
                    <div class="form-group">
                        <label for="name">"Repository name"</label>
                        <input type="text" id="name" name="name" required pattern="[a-zA-Z0-9._-]+" />
                    </div>
                    <div class="form-group">
                        <label for="description">"Description (optional)"</label>
                        <input type="text" id="description" name="description" />
                    </div>
                    <div class="form-group form-inline">
                        <input type="hidden" name="is_private" value="false" />
                        <input type="checkbox" id="is_private" name="is_private" value="true" class="form-checkbox" />
                        <label for="is_private">"Private repository"</label>
                    </div>
                    <button type="submit" class="btn btn-primary btn-full">
                        "Create repository"
                    </button>
                </ActionForm>
            </div>
        </div>
    }
}
