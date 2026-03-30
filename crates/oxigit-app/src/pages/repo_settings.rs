use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollaboratorInfo {
    pub user_id: i64,
    pub username: String,
    pub permission: String,
    pub created_at: String,
}

#[server]
async fn list_collaborators(
    owner: String,
    repo: String,
) -> Result<Vec<CollaboratorInfo>, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;

    let (_repo_owner, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can manage collaborators"));
    }

    let collabs = db::list_collaborators(&pool, repo_db.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(collabs
        .into_iter()
        .map(|(u, c)| CollaboratorInfo {
            user_id: u.id,
            username: u.username,
            permission: c.permission,
            created_at: c.created_at,
        })
        .collect())
}

#[server]
async fn add_collaborator(
    owner: String,
    repo: String,
    username: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can add collaborators"));
    }

    let target_user = db::get_user_by_username(&pool, &username)
        .await
        .map_err(|_| ServerFnError::new("User not found"))?;

    if target_user.id == repo_db.owner_id {
        return Err(ServerFnError::new("Cannot add the owner as a collaborator"));
    }

    db::add_collaborator(&pool, repo_db.id, target_user.id, "write")
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn remove_collaborator(
    owner: String,
    repo: String,
    user_id: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can remove collaborators"));
    }

    db::remove_collaborator(&pool, repo_db.id, user_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[component]
pub fn RepoSettingsPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let collabs = Resource::new(
        move || (owner(), repo()),
        move |(o, r)| list_collaborators(o, r),
    );

    let add_action = ServerAction::<AddCollaborator>::new();
    let remove_action = ServerAction::<RemoveCollaborator>::new();

    Effect::new(move || {
        add_action.version().get();
        remove_action.version().get();
        collabs.refetch();
    });

    let error = move || {
        add_action.value().get().and_then(|r| r.err().map(|e| e.to_string()))
    };

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"Settings"</span>
            </h1>
        </div>

        <div class="card">
            <div class="card-header">"Collaborators"</div>
            <p class="text-secondary mb-4" style="font-size: 0.875rem;">
                "Collaborators have write (push) access to this repository."
            </p>

            {move || error().map(|e| view! {
                <div class="flash flash-error">{e}</div>
            })}

            <ActionForm action=add_action>
                <input type="hidden" name="owner" value={move || owner()} />
                <input type="hidden" name="repo" value={move || repo()} />
                <div class="form-inline mb-4">
                    <input
                        type="text"
                        name="username"
                        required
                        placeholder="Username"
                        class="form-input flex-1"
                    />
                    <button type="submit" class="btn btn-primary">"Add"</button>
                </div>
            </ActionForm>

            <Suspense fallback=|| view! { <p class="text-secondary">"Loading..."</p> }>
                {move || {
                    let owner_name = owner();
                    let repo_name = repo();
                    Suspend::new(async move {
                        match collabs.await {
                            Ok(collabs) if collabs.is_empty() => view! {
                                <p class="text-secondary">"No collaborators yet."</p>
                            }.into_any(),
                            Ok(collabs) => {
                                let on = owner_name.clone();
                                let rn = repo_name.clone();
                                view! {
                                <ul class="list">
                                    {collabs.into_iter().map(|c| {
                                        let username = c.username.clone();
                                        let perm = c.permission.clone();
                                        let uid = c.user_id;
                                        let o = on.clone();
                                        let r = rn.clone();
                                        view! {
                                            <li class="list-item">
                                                <div class="flex-row gap-2">
                                                    <span class="font-semibold">{username}</span>
                                                    <span class="badge badge-private">{perm}</span>
                                                </div>
                                                <ActionForm action=remove_action>
                                                    <input type="hidden" name="owner" value={o} />
                                                    <input type="hidden" name="repo" value={r} />
                                                    <input type="hidden" name="user_id" value={uid.to_string()} />
                                                    <button type="submit" class="btn btn-danger btn-sm">"Remove"</button>
                                                </ActionForm>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any()},
                            Err(e) => view! {
                                <div class="flash flash-error">{e.to_string()}</div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
