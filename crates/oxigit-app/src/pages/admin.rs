use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminUserInfo {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub is_disabled: bool,
    pub plan: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminContactInquiry {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub company: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminDashboardData {
    pub user_count: i64,
    pub repo_count: i64,
    pub subscription_breakdown: Vec<(String, i64)>,
    pub disk_usage_display: String,
    pub users: Vec<AdminUserInfo>,
    pub contact_inquiries: Vec<AdminContactInquiry>,
}

#[server]
async fn get_admin_dashboard() -> Result<AdminDashboardData, ServerFnError> {
    use crate::server_fns::{get_data_dir, get_pool, require_admin};
    use oxigit_core::db;

    require_admin().await?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    let user_count = db::count_users(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let repo_count = db::count_repositories(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    #[cfg(feature = "saas")]
    let subscription_breakdown = db::subscription_breakdown(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    #[cfg(not(feature = "saas"))]
    let subscription_breakdown = vec![];

    // Compute disk usage of data directory
    let disk_usage_display = {
        fn dir_size(path: &std::path::Path) -> u64 {
            let mut total = 0u64;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let ft = entry.file_type();
                    if let Ok(ft) = ft {
                        if ft.is_file() {
                            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                        } else if ft.is_dir() {
                            total += dir_size(&entry.path());
                        }
                    }
                }
            }
            total
        }
        let bytes = dir_size(&data_dir);
        if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    };

    // Fetch all users with their plans
    let all_users = db::list_all_users(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut users = Vec::with_capacity(all_users.len());
    for u in all_users {
        let plan = db::get_user_plan(&pool, u.id)
            .await
            .unwrap_or_else(|_| "free".to_string());
        users.push(AdminUserInfo {
            id: u.id,
            username: u.username,
            email: u.email,
            is_admin: u.is_admin,
            is_disabled: u.is_disabled,
            plan,
            created_at: u.created_at,
        });
    }

    let inquiries = db::list_contact_inquiries(&pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|i| AdminContactInquiry {
            id: i.id,
            name: i.name,
            email: i.email,
            company: i.company,
            message: i.message,
            created_at: i.created_at,
        })
        .collect();

    Ok(AdminDashboardData {
        user_count,
        repo_count,
        subscription_breakdown,
        disk_usage_display,
        users,
        contact_inquiries: inquiries,
    })
}

#[server]
async fn admin_toggle_disabled(user_id: i64, disabled: bool) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_pool, require_admin};
    use oxigit_core::db;

    require_admin().await?;
    let pool = get_pool().await?;
    db::set_user_disabled(&pool, user_id, disabled)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn admin_set_plan(user_id: i64, plan: String) -> Result<(), ServerFnError> {
    use crate::server_fns::{require_admin};

    require_admin().await?;
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::get_pool;
        use oxigit_core::db;
        let pool = get_pool().await?;
        db::admin_override_plan(&pool, user_id, &plan)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    #[cfg(not(feature = "saas"))]
    {
        let _ = (user_id, plan);
        return Err(ServerFnError::new("Plan management requires the SaaS edition"));
    }
    Ok(())
}

#[component]
pub fn AdminPage() -> impl IntoView {
    let dashboard = Resource::new(|| (), |_| get_admin_dashboard());

    let toggle_action = Action::new(move |input: &(i64, bool)| {
        let (user_id, disabled) = *input;
        async move { admin_toggle_disabled(user_id, disabled).await }
    });

    let plan_action = Action::new(move |input: &(i64, String)| {
        let (user_id, plan) = input.clone();
        async move { admin_set_plan(user_id, plan).await }
    });

    view! {
        <div class="admin-page">
            <h1>"Admin Dashboard"</h1>
            <Suspense fallback=move || view! { <LoadingPage /> }>
                {move || dashboard.get().map(|result| match result {
                    Err(e) => view! {
                        <ErrorDisplay error=e.to_string() />
                    }.into_any(),
                    Ok(data) => {
                        let users = data.users.clone();
                        view! {
                            <div class="admin-stats">
                                <div class="stat-card">
                                    <span class="stat-value">{data.user_count}</span>
                                    <span class="stat-label">"Users"</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-value">{data.repo_count}</span>
                                    <span class="stat-label">"Repositories"</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-value">{data.disk_usage_display.clone()}</span>
                                    <span class="stat-label">"Disk Usage"</span>
                                </div>
                            </div>

                            <h2>"Subscription Breakdown"</h2>
                            <div class="admin-stats">
                                {data.subscription_breakdown.iter().map(|(plan, count)| {
                                    let plan = plan.clone();
                                    let count = *count;
                                    view! {
                                        <div class="stat-card">
                                            <span class="stat-value">{count}</span>
                                            <span class="stat-label">{plan}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>

                            <h2>"Users"</h2>
                            <table class="admin-table">
                                <thead>
                                    <tr>
                                        <th>"Username"</th>
                                        <th>"Email"</th>
                                        <th>"Plan"</th>
                                        <th>"Status"</th>
                                        <th>"Admin"</th>
                                        <th>"Joined"</th>
                                        <th>"Actions"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {users.into_iter().map(|u| {
                                        let user_id = u.id;
                                        let is_disabled = u.is_disabled;
                                        let current_plan = u.plan.clone();
                                        view! {
                                            <tr class={if u.is_disabled { "user-disabled" } else { "" }}>
                                                <td><a href={format!("/{}", u.username)}>{u.username.clone()}</a></td>
                                                <td>{u.email.clone()}</td>
                                                <td>
                                                    <select
                                                        on:change=move |ev| {
                                                            let val = event_target_value(&ev);
                                                            plan_action.dispatch((user_id, val));
                                                        }
                                                    >
                                                        <option value="free" selected={current_plan == "free"}>"Free"</option>
                                                        <option value="flat" selected={current_plan == "flat"}>"Flat"</option>
                                                        <option value="team" selected={current_plan == "team"}>"Team"</option>
                                                        <option value="founding" selected={current_plan == "founding"}>"Founding"</option>
                                                        <option value="enterprise" selected={current_plan == "enterprise"}>"Enterprise"</option>
                                                    </select>
                                                </td>
                                                <td>{if u.is_disabled { "Disabled" } else { "Active" }}</td>
                                                <td>{if u.is_admin { "Yes" } else { "No" }}</td>
                                                <td>{u.created_at.split('T').next().unwrap_or(&u.created_at).to_string()}</td>
                                                <td>
                                                    <button
                                                        class={if is_disabled { "btn btn-sm" } else { "btn btn-sm btn-danger" }}
                                                        on:click=move |_| {
                                                            toggle_action.dispatch((user_id, !is_disabled));
                                                        }
                                                    >
                                                        {if is_disabled { "Enable" } else { "Disable" }}
                                                    </button>
                                                </td>
                                            </tr>
                                        }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                            {if !data.contact_inquiries.is_empty() {
                                let inquiries = data.contact_inquiries.clone();
                                Some(view! {
                                    <h2>"Contact Inquiries"</h2>
                                    <table class="admin-table">
                                        <thead>
                                            <tr>
                                                <th>"Name"</th>
                                                <th>"Email"</th>
                                                <th>"Company"</th>
                                                <th>"Message"</th>
                                                <th>"Date"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {inquiries.into_iter().map(|inq| {
                                                view! {
                                                    <tr>
                                                        <td>{inq.name}</td>
                                                        <td>{inq.email}</td>
                                                        <td>{inq.company}</td>
                                                        <td style="max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{inq.message}</td>
                                                        <td>{inq.created_at.split('T').next().unwrap_or(&inq.created_at).to_string()}</td>
                                                    </tr>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </tbody>
                                    </table>
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
