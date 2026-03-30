use leptos::prelude::*;

#[server]
async fn create_repo(
    name: String,
    description: String,
    is_private: bool,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    db::create_repository(&pool, user.id, &name, &description, is_private, &data_dir)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

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
                    <div class="flash flash-error">{e}</div>
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
