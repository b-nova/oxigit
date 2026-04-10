use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;
use crate::components::toast::use_toast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrgMemberInfo {
    pub user_id: i64,
    pub username: String,
    pub role: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrgSettingsData {
    pub slug: String,
    pub display_name: String,
    pub members: Vec<OrgMemberInfo>,
    pub is_owner: bool,
}

#[server]
async fn get_org_settings(slug: String) -> Result<OrgSettingsData, ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{
            extract_session_user, get_control_pool, get_user_entitlements, sfn_err,
        };
        use oxigit_core::db;

        let user = extract_session_user()
            .await
            .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

        let entitlements = get_user_entitlements(user.id).await?;
        if !entitlements.team_features {
            return Err(ServerFnError::new(
                "Team management requires a Team plan. Upgrade at /pricing",
            ));
        }

        let pool = get_control_pool().await?;

        let org = db::get_organization_by_slug(&pool, &slug)
            .await
            .map_err(sfn_err)?;

        let membership = db::get_org_membership(&pool, org.id, user.id)
            .await
            .map_err(sfn_err)?;

        if membership.is_none() {
            return Err(ServerFnError::new("Not a member of this organization"));
        }

        let is_owner = membership
            .as_ref()
            .map(|m| m.role == "owner")
            .unwrap_or(false);

        let members_raw = db::list_org_members(&pool, org.id).await.map_err(sfn_err)?;

        let members = members_raw
            .into_iter()
            .map(|(u, role)| OrgMemberInfo {
                user_id: u.id,
                username: u.username,
                role,
            })
            .collect();

        Ok(OrgSettingsData {
            slug: org.slug,
            display_name: org.display_name,
            members,
            is_owner,
        })
    }
    #[cfg(not(feature = "saas"))]
    {
        let _ = slug;
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[server]
async fn add_member(slug: String, username: String, role: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{
            extract_session_user, get_control_pool, get_user_entitlements, sfn_err,
        };
        use oxigit_core::db;

        let user = extract_session_user()
            .await
            .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

        let entitlements = get_user_entitlements(user.id).await?;
        if !entitlements.team_features {
            return Err(ServerFnError::new(
                "Team management requires a Team plan. Upgrade at /pricing",
            ));
        }

        let pool = get_control_pool().await?;

        let org = db::get_organization_by_slug(&pool, &slug)
            .await
            .map_err(sfn_err)?;

        // Only owners can add members
        let membership = db::get_org_membership(&pool, org.id, user.id)
            .await
            .map_err(sfn_err)?;
        if membership.map(|m| m.role != "owner").unwrap_or(true) {
            return Err(ServerFnError::new("Only org owners can add members"));
        }

        let target = db::get_user_by_username(&pool, &username)
            .await
            .map_err(|_| ServerFnError::new("User not found"))?;

        db::add_org_member(&pool, org.id, target.id, &role)
            .await
            .map_err(sfn_err)?;

        Ok(())
    }
    #[cfg(not(feature = "saas"))]
    {
        let _ = slug;
        let _ = username;
        let _ = role;
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[server]
async fn remove_member(slug: String, user_id: i64) -> Result<(), ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{
            extract_session_user, get_control_pool, get_user_entitlements, sfn_err,
        };
        use oxigit_core::db;

        let user = extract_session_user()
            .await
            .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

        let entitlements = get_user_entitlements(user.id).await?;
        if !entitlements.team_features {
            return Err(ServerFnError::new(
                "Team management requires a Team plan. Upgrade at /pricing",
            ));
        }

        let pool = get_control_pool().await?;

        let org = db::get_organization_by_slug(&pool, &slug)
            .await
            .map_err(sfn_err)?;

        let membership = db::get_org_membership(&pool, org.id, user.id)
            .await
            .map_err(sfn_err)?;
        if membership.map(|m| m.role != "owner").unwrap_or(true) {
            return Err(ServerFnError::new("Only org owners can remove members"));
        }

        if user_id == user.id {
            return Err(ServerFnError::new("Cannot remove yourself"));
        }

        db::remove_org_member(&pool, org.id, user_id)
            .await
            .map_err(sfn_err)?;

        Ok(())
    }
    #[cfg(not(feature = "saas"))]
    {
        let _ = slug;
        let _ = user_id;
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[component]
pub fn OrgSettingsPage() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let settings = Resource::new(slug, get_org_settings);

    let add_action = ServerAction::<AddMember>::new();
    let remove_action = Action::new(move |input: &(String, i64)| {
        let (slug, user_id) = input.clone();
        async move { remove_member(slug, user_id).await }
    });

    let toast = use_toast();
    let toast_add = toast.clone();
    let toast_remove = toast.clone();
    Effect::new(move |_| {
        if let Some(Ok(_)) = add_action.value().get() {
            toast_add.success("Member added");
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(_)) = remove_action.value().get() {
            toast_remove.success("Member removed");
        }
    });

    view! {
        <div class="settings-page">
            <Suspense fallback=move || view! { <LoadingPage /> }>
                {move || settings.get().map(|result| match result {
                    Err(e) => view! {
                        <ErrorDisplay error=e.to_string() />
                    }.into_any(),
                    Ok(data) => {
                        let members = data.members.clone();
                        let is_owner = data.is_owner;
                        let org_slug = data.slug.clone();
                        view! {
                            <h1>{format!("{} — Settings", data.display_name)}</h1>

                            <h2>"Members"</h2>
                            <table class="admin-table">
                                <thead>
                                    <tr>
                                        <th>"Username"</th>
                                        <th>"Role"</th>
                                        {if is_owner { Some(view! { <th>"Actions"</th> }) } else { None }}
                                    </tr>
                                </thead>
                                <tbody>
                                    {members.into_iter().map(|m| {
                                        let member_slug = org_slug.clone();
                                        let member_id = m.user_id;
                                        view! {
                                            <tr>
                                                <td><a href={format!("/{}", m.username)}>{m.username.clone()}</a></td>
                                                <td>{m.role.clone()}</td>
                                                {is_owner.then(|| {
                                                    let can_remove = m.role != "owner";
                                                    view! {
                                                        <td>
                                                            {can_remove.then(|| view! {
                                                                <button
                                                                    class="btn btn-sm btn-danger"
                                                                    on:click=move |_| {
                                                                        remove_action.dispatch((member_slug.clone(), member_id));
                                                                    }
                                                                >"Remove"</button>
                                                            })}
                                                        </td>
                                                    }
                                                })}
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>

                            {if is_owner {
                                let form_slug = data.slug.clone();
                                Some(view! {
                                    <h2>"Add Member"</h2>
                                    <ActionForm action=add_action>
                                        <input type="hidden" name="slug" value={form_slug} />
                                        <div class="form-row">
                                            <div class="form-group">
                                                <label for="add-username">"Username"</label>
                                                <input type="text" id="add-username" name="username" required />
                                            </div>
                                            <div class="form-group">
                                                <label for="add-role">"Role"</label>
                                                <select id="add-role" name="role">
                                                    <option value="member">"Member"</option>
                                                    <option value="admin">"Admin"</option>
                                                </select>
                                            </div>
                                            <button type="submit" class="btn btn-primary">"Add"</button>
                                        </div>
                                    </ActionForm>
                                })
                            } else {
                                None
                            }}
                        }.into_any()
                    }
                })}
            </Suspense>
        </div>
    }
}
