use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HookStatus {
    tool_id: String,
    up_to_date: bool,
}

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

#[cfg(feature = "ssr")]
fn hook_script(tool_name: &str, extra_session: &str) -> String {
    format!(
        r#"#!/bin/bash
set -e
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')
if ! echo "$COMMAND" | grep -qE "^git commit"; then exit 0; fi
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
{extra_session}MODEL=$(echo "$INPUT" | jq -r '.model // empty')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')
PROMPT=""
if [ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ]; then
  PROMPT=$(jq -r '[.[] | select(.type == "human")] | last | .message.content[]? | select(.type == "text") | .text' "$TRANSCRIPT" 2>/dev/null | head -c 500 || true)
fi
mkdir -p .oxigit
jq -n --arg tool "{tool_name}" --arg model "$MODEL" --arg session_id "$SESSION_ID" --arg prompt "$PROMPT" \
  '{{ tool: $tool,
     model: (if $model == "" then null else $model end),
     session_id: (if $session_id == "" then null else $session_id end),
     prompt: (if $prompt == "" then null else $prompt end) }}' \
  > .oxigit/context.json
git add .oxigit/context.json"#,
        tool_name = tool_name,
        extra_session = extra_session,
    )
}

#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
struct AiToolConfig {
    tool_id: &'static str,
    display_name: &'static str,
    hook_dir: &'static str,
    config_path: &'static str,
    config_content: &'static str,
    extra_session: &'static str,
}

const AI_TOOLS: &[AiToolConfig] = &[
    AiToolConfig {
        tool_id: "claude-code",
        display_name: "Claude Code",
        hook_dir: ".claude/hooks",
        config_path: ".claude/settings.json",
        config_content: r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/oxigit-context.sh",
            "timeout": 10
          }
        ]
      }
    ]
  }
}"#,
        extra_session: "",
    },
    AiToolConfig {
        tool_id: "codex",
        display_name: "Codex CLI",
        hook_dir: ".codex/hooks",
        config_path: ".codex/hooks.json",
        config_content: r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": ".codex/hooks/oxigit-context.sh",
            "timeout": 10
          }
        ]
      }
    ]
  }
}"#,
        extra_session: "",
    },
    AiToolConfig {
        tool_id: "gemini-cli",
        display_name: "Gemini CLI",
        hook_dir: ".gemini/hooks",
        config_path: ".gemini/settings.json",
        config_content: r#"{
  "hooks": {
    "BeforeTool": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": ".gemini/hooks/oxigit-context.sh",
            "timeout": 10
          }
        ]
      }
    ]
  }
}"#,
        extra_session: "if [ -z \"$SESSION_ID\" ] && [ -n \"$GEMINI_SESSION_ID\" ]; then\n  SESSION_ID=\"$GEMINI_SESSION_ID\"\nfi\n",
    },
];

#[cfg(feature = "ssr")]
fn get_tool_config(tool_id: &str) -> Option<&'static AiToolConfig> {
    AI_TOOLS.iter().find(|t| t.tool_id == tool_id)
}

#[server]
async fn install_ai_hook(
    owner: String,
    repo: String,
    tool_id: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can install AI hooks"));
    }

    let tool = get_tool_config(&tool_id)
        .ok_or_else(|| ServerFnError::new("Unknown AI tool"))?;

    let repo_path = git::repo_path(&data_dir, &owner, &repo);

    let branch = git::default_branch(&repo_path)
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Repository has no branches"))?;

    let script_content = hook_script(tool.tool_id, tool.extra_session);
    let script_path = format!("{}/oxigit-context.sh", tool.hook_dir);

    let files: Vec<(&str, &str, bool)> = vec![
        (&script_path, &script_content, true),
        (tool.config_path, tool.config_content, false),
    ];

    git::add_files_to_branch(
        &repo_path,
        &branch,
        &files,
        &format!("chore: install {} AI hook for Oxigit", tool.display_name),
        &user.username,
        &format!("{}@oxigit", user.username),
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn check_installed_hooks(
    owner: String,
    repo: String,
) -> Result<Vec<HookStatus>, ServerFnError> {
    use crate::server_fns::get_data_dir;
    use oxigit_core::git;

    let data_dir = get_data_dir().await?;
    let repo_path = git::repo_path(&data_dir, &owner, &repo);

    let branch = match git::default_branch(&repo_path) {
        Ok(Some(b)) => b,
        _ => return Ok(vec![]),
    };

    let mut statuses = Vec::new();
    for tool in AI_TOOLS {
        let script_path = format!("{}/oxigit-context.sh", tool.hook_dir);
        if let Ok(installed_content) = git::read_blob(&repo_path, &branch, &script_path) {
            let expected = hook_script(tool.tool_id, tool.extra_session);
            let up_to_date = String::from_utf8_lossy(&installed_content).as_ref() == expected;
            statuses.push(HookStatus {
                tool_id: tool.tool_id.to_string(),
                up_to_date,
            });
        }
    }
    Ok(statuses)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebhookInfo {
    pub id: i64,
    pub url: String,
    pub active: bool,
    pub created_at: String,
}

#[server]
async fn list_repo_webhooks(
    owner: String,
    repo: String,
) -> Result<Vec<WebhookInfo>, ServerFnError> {
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
        return Err(ServerFnError::new("Not authorized"));
    }

    let hooks = db::list_webhooks(&pool, repo_db.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(hooks.into_iter().map(|h| WebhookInfo {
        id: h.id,
        url: h.url,
        active: h.active,
        created_at: h.created_at,
    }).collect())
}

#[server]
async fn add_webhook(
    owner: String,
    repo: String,
    url: String,
    secret: String,
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
        return Err(ServerFnError::new("Not authorized"));
    }

    let secret = if secret.is_empty() { None } else { Some(secret) };
    db::create_webhook(&pool, repo_db.id, &url, secret.as_deref())
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn delete_webhook(
    owner: String,
    repo: String,
    webhook_id: i64,
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
        return Err(ServerFnError::new("Not authorized"));
    }

    db::delete_webhook(&pool, webhook_id, repo_db.id)
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
    let install_hook_action = ServerAction::<InstallAiHook>::new();
    let add_webhook_action = ServerAction::<AddWebhook>::new();
    let delete_webhook_action = ServerAction::<DeleteWebhook>::new();

    let webhooks = Resource::new(
        move || (owner(), repo()),
        move |(o, r)| list_repo_webhooks(o, r),
    );

    let installed_hooks = Resource::new(
        move || (owner(), repo()),
        move |(o, r)| check_installed_hooks(o, r),
    );

    Effect::new(move || {
        add_action.version().get();
        remove_action.version().get();
        collabs.refetch();
    });

    Effect::new(move || {
        install_hook_action.version().get();
        installed_hooks.refetch();
    });

    Effect::new(move || {
        add_webhook_action.version().get();
        delete_webhook_action.version().get();
        webhooks.refetch();
    });

    let error = move || {
        add_action.value().get().and_then(|r| r.err().map(|e| e.to_string()))
    };

    let hook_error = move || {
        install_hook_action.value().get().and_then(|r| r.err().map(|e| e.to_string()))
    };
    let hook_success = move || {
        install_hook_action.value().get().and_then(|r| r.ok()).is_some()
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

        <div class="card">
            <div class="card-header">"AI Hooks"</div>
            <p class="text-secondary mb-4" style="font-size: 0.875rem;">
                "Install AI coding tool hooks to automatically track AI-generated commits in the "
                <a href={move || format!("/{}/{}/ai-timeline", owner(), repo())}>"AI Timeline"</a>
                ". Clicking install commits the hook files directly to the default branch."
            </p>
            {move || hook_error().map(|e| view! {
                <div class="flash flash-error">{e}</div>
            })}
            {move || hook_success().then(|| view! {
                <div class="flash flash-success">"AI hook installed successfully. Pull to get the new files."</div>
            })}
            {AI_TOOLS.iter().map(|tool| {
                let tool_id = tool.tool_id.to_string();
                let tool_id_check = tool.tool_id.to_string();
                let display_name = tool.display_name.to_string();
                let hook_dir = tool.hook_dir.to_string();
                let config_path = tool.config_path.to_string();
                // None = not installed, Some(true) = up to date, Some(false) = outdated
                let hook_status = Memo::new(move |_| {
                    installed_hooks.get()
                        .and_then(|r| r.ok())
                        .and_then(|statuses| {
                            statuses.iter()
                                .find(|s| s.tool_id == tool_id_check)
                                .map(|s| s.up_to_date)
                        })
                });
                view! {
                    <div class="list-item">
                        <div>
                            <span class="font-semibold">{display_name}</span>
                            <span class="text-secondary" style="font-size: 0.8125rem; margin-left: 0.5rem;">
                                {format!("{}/oxigit-context.sh + {}", hook_dir, config_path)}
                            </span>
                        </div>
                        <ActionForm action=install_hook_action>
                            <input type="hidden" name="owner" value={move || owner()} />
                            <input type="hidden" name="repo" value={move || repo()} />
                            <input type="hidden" name="tool_id" value={tool_id} />
                            <button
                                type="submit"
                                class="btn btn-sm"
                                class:btn-primary=move || !matches!(hook_status.get(), Some(true))
                                disabled=move || hook_status.get() == Some(true)
                            >
                                {move || match hook_status.get() {
                                    None => "Install",
                                    Some(true) => "Installed",
                                    Some(false) => "Update",
                                }}
                            </button>
                        </ActionForm>
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>

        // Webhooks section
        <div class="card mt-6">
            <div class="card-header">"Webhooks"</div>
            <div style="padding: var(--space-4);">
                <p class="text-secondary mb-3" style="font-size: 0.8125rem;">
                    "Configure webhooks to trigger deploy previews on push. Select a provider template or enter a custom URL."
                </p>

                // Provider templates
                <div class="form-group">
                    <label>"Quick setup"</label>
                    <div class="webhook-templates">
                        <button
                            type="button"
                            class="btn btn-sm"
                            on:click=move |_| {
                                if let Some(el) = document().get_element_by_id("webhook_url") {
                                    let _ = el.set_attribute("value", "https://api.vercel.com/v1/integrations/deploy/");
                                }
                                if let Some(el) = document().get_element_by_id("webhook_url_hint") {
                                    el.set_text_content(Some("Paste your Vercel Deploy Hook URL from Project Settings > Git > Deploy Hooks"));
                                }
                            }
                        >"Vercel"</button>
                        <button
                            type="button"
                            class="btn btn-sm"
                            on:click=move |_| {
                                if let Some(el) = document().get_element_by_id("webhook_url") {
                                    let _ = el.set_attribute("value", "https://api.netlify.com/build_hooks/");
                                }
                                if let Some(el) = document().get_element_by_id("webhook_url_hint") {
                                    el.set_text_content(Some("Paste your Netlify Build Hook URL from Site Settings > Build & Deploy > Build hooks"));
                                }
                            }
                        >"Netlify"</button>
                        <button
                            type="button"
                            class="btn btn-sm"
                            on:click=move |_| {
                                if let Some(el) = document().get_element_by_id("webhook_url") {
                                    let _ = el.set_attribute("value", "https://api.cloudflare.com/client/v4/pages/webhooks/deploy_hooks/");
                                }
                                if let Some(el) = document().get_element_by_id("webhook_url_hint") {
                                    el.set_text_content(Some("Paste your Cloudflare Pages Deploy Hook URL"));
                                }
                            }
                        >"Cloudflare Pages"</button>
                        <button
                            type="button"
                            class="btn btn-sm"
                            on:click=move |_| {
                                if let Some(el) = document().get_element_by_id("webhook_url") {
                                    let _ = el.set_attribute("value", "https://api.digitalocean.com/v2/apps//deployments");
                                }
                                if let Some(el) = document().get_element_by_id("webhook_url_hint") {
                                    el.set_text_content(Some("Replace <app_id> in the URL. Use your DO API token as the webhook secret for Bearer auth."));
                                }
                            }
                        >"DigitalOcean"</button>
                        <button
                            type="button"
                            class="btn btn-sm"
                            on:click=move |_| {
                                if let Some(el) = document().get_element_by_id("webhook_url") {
                                    let _ = el.set_attribute("value", "https://api.machines.dev/v1/apps//machines//restart");
                                }
                                if let Some(el) = document().get_element_by_id("webhook_url_hint") {
                                    el.set_text_content(Some("Replace <app_name> and <machine_id>. Use your Fly.io auth token as the webhook secret."));
                                }
                            }
                        >"Fly.io"</button>
                        <button
                            type="button"
                            class="btn btn-sm"
                            on:click=move |_| {
                                if let Some(el) = document().get_element_by_id("webhook_url") {
                                    let _ = el.set_attribute("value", "");
                                }
                                if let Some(el) = document().get_element_by_id("webhook_url_hint") {
                                    el.set_text_content(Some("Enter any URL that accepts POST requests with JSON payload"));
                                }
                            }
                        >"Custom"</button>
                    </div>
                </div>

                <ActionForm action=add_webhook_action>
                    <input type="hidden" name="owner" value={move || owner()} />
                    <input type="hidden" name="repo" value={move || repo()} />
                    <div class="form-group">
                        <label for="webhook_url">"Webhook URL"</label>
                        <input type="url" id="webhook_url" name="url" required placeholder="https://api.vercel.com/v1/integrations/deploy/..." />
                        <p id="webhook_url_hint" class="text-tertiary" style="font-size: 0.75rem; margin-top: var(--space-1);"></p>
                    </div>
                    <div class="form-group">
                        <label for="webhook_secret">"Secret (optional)"</label>
                        <input type="text" id="webhook_secret" name="secret" placeholder="For HMAC signing" />
                    </div>
                    <button type="submit" class="btn btn-primary btn-sm">"Add webhook"</button>
                </ActionForm>
            </div>

            <Suspense fallback=|| ()>
                {move || {
                    let on = owner();
                    let rn = repo();
                    Suspend::new(async move {
                        match webhooks.await {
                            Ok(hooks) if hooks.is_empty() => view! {
                                <p class="text-secondary" style="padding: 0 var(--space-4) var(--space-4);">"No webhooks configured."</p>
                            }.into_any(),
                            Ok(hooks) => {
                            let hook_owner = on.clone();
                            let hook_repo = rn.clone();
                            view! {
                                <ul class="list">
                                    {hooks.into_iter().map(|hook| {
                                        let url = hook.url.clone();
                                        let hid = hook.id;
                                        let ho = hook_owner.clone();
                                        let hr = hook_repo.clone();
                                        view! {
                                            <li class="list-item">
                                                <div>
                                                    <code class="font-mono" style="font-size: 0.8125rem;">{url}</code>
                                                    <div class="list-item-meta">"Added " {hook.created_at.clone()}</div>
                                                </div>
                                                <ActionForm action=delete_webhook_action>
                                                    <input type="hidden" name="owner" value={ho} />
                                                    <input type="hidden" name="repo" value={hr} />
                                                    <input type="hidden" name="webhook_id" value={hid.to_string()} />
                                                    <button type="submit" class="btn btn-danger btn-sm">"Delete"</button>
                                                </ActionForm>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any()
                        }
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
