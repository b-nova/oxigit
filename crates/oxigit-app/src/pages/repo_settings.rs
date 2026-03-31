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
fn save_prompt_script() -> &'static str {
    r#"#!/bin/bash
set -e
INPUT=$(cat)
PROMPT=$(echo "$INPUT" | jq -r '.prompt // empty')
if [ -n "$PROMPT" ]; then
  mkdir -p .oxigit
  printf '%s' "$PROMPT" > .oxigit/last-prompt.txt
fi"#
}

#[cfg(feature = "ssr")]
fn hook_script(tool_name: &str, has_model: bool) -> String {
    let model_line = if has_model {
        r#"MODEL=$(echo "$INPUT" | jq -r '.model // empty')"#
    } else {
        r#"MODEL="""#
    };
    format!(
        r#"#!/bin/bash
set -e
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')
if ! echo "$COMMAND" | grep -q "git commit"; then exit 0; fi
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
{model_line}
PROMPT=""
if [ -f .oxigit/last-prompt.txt ]; then
  PROMPT=$(head -c 500 .oxigit/last-prompt.txt)
else
  TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')
  if [ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ]; then
    PROMPT=$(jq -s -r '[.[] | select(.type == "human")] | last | .message.content[]? | select(.type == "text") | .text' "$TRANSCRIPT" 2>/dev/null | head -c 500 || true)
  fi
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
        model_line = model_line,
    )
}

#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
struct AiToolConfig {
    tool_id: &'static str,
    display_name: &'static str,
    hook_dir: &'static str,
    config_path: &'static str,
    has_model: bool,
    prompt_event: &'static str,
    tool_event: &'static str,
    tool_matcher: &'static str,
}

#[cfg(feature = "ssr")]
fn config_json(tool: &AiToolConfig) -> String {
    format!(
        r#"{{
  "hooks": {{
    "{prompt_event}": [
      {{
        "matcher": "",
        "hooks": [
          {{
            "type": "command",
            "command": "{hook_dir}/save-prompt.sh",
            "timeout": 5
          }}
        ]
      }}
    ],
    "{tool_event}": [
      {{
        "matcher": "{tool_matcher}",
        "hooks": [
          {{
            "type": "command",
            "command": "{hook_dir}/oxigit-context.sh",
            "timeout": 10
          }}
        ]
      }}
    ]
  }}
}}"#,
        prompt_event = tool.prompt_event,
        tool_event = tool.tool_event,
        tool_matcher = tool.tool_matcher,
        hook_dir = tool.hook_dir,
    )
}

const AI_TOOLS: &[AiToolConfig] = &[
    AiToolConfig {
        tool_id: "claude-code",
        display_name: "Claude Code",
        hook_dir: ".claude/hooks",
        config_path: ".claude/settings.json",
        has_model: true,
        prompt_event: "UserPromptSubmit",
        tool_event: "PreToolUse",
        tool_matcher: "Bash",
    },
    AiToolConfig {
        tool_id: "codex",
        display_name: "Codex CLI",
        hook_dir: ".codex/hooks",
        config_path: ".codex/hooks.json",
        has_model: true,
        prompt_event: "UserPromptSubmit",
        tool_event: "PreToolUse",
        tool_matcher: "Bash",
    },
    AiToolConfig {
        tool_id: "gemini-cli",
        display_name: "Gemini CLI",
        hook_dir: ".gemini/hooks",
        config_path: ".gemini/settings.json",
        has_model: false,
        prompt_event: "BeforeAgent",
        tool_event: "BeforeTool",
        tool_matcher: "run_shell_command",
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

    let save_prompt = save_prompt_script().to_string();
    let context_script = hook_script(tool.tool_id, tool.has_model);
    let config = config_json(tool);
    let save_prompt_path = format!("{}/save-prompt.sh", tool.hook_dir);
    let context_path = format!("{}/oxigit-context.sh", tool.hook_dir);

    let files: Vec<(&str, &str, bool)> = vec![
        (&save_prompt_path, &save_prompt, true),
        (&context_path, &context_script, true),
        (tool.config_path, &config, false),
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
        let context_path = format!("{}/oxigit-context.sh", tool.hook_dir);
        let prompt_path = format!("{}/save-prompt.sh", tool.hook_dir);

        let context_installed = git::read_blob(&repo_path, &branch, &context_path).ok();
        let prompt_installed = git::read_blob(&repo_path, &branch, &prompt_path).ok();

        // Consider installed if either script exists
        if context_installed.is_some() || prompt_installed.is_some() {
            let context_ok = context_installed
                .map(|b| String::from_utf8_lossy(&b).as_ref() == hook_script(tool.tool_id, tool.has_model))
                .unwrap_or(false);
            let prompt_ok = prompt_installed
                .map(|b| String::from_utf8_lossy(&b).as_ref() == save_prompt_script())
                .unwrap_or(false);
            let config_ok = git::read_blob(&repo_path, &branch, tool.config_path)
                .map(|b| String::from_utf8_lossy(&b).as_ref() == config_json(tool))
                .unwrap_or(false);

            statuses.push(HookStatus {
                tool_id: tool.tool_id.to_string(),
                up_to_date: context_ok && prompt_ok && config_ok,
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
async fn fetch_repo_visibility(
    owner: String,
    repo: String,
) -> Result<bool, ServerFnError> {
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

    Ok(repo_db.is_private)
}

#[server]
async fn update_visibility(
    owner: String,
    repo: String,
    is_private: bool,
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
        return Err(ServerFnError::new("Only the owner can change visibility"));
    }

    db::update_repository_visibility(&pool, repo_db.id, is_private)
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

    let visibility = Resource::new(
        move || (owner(), repo()),
        move |(o, r)| fetch_repo_visibility(o, r),
    );
    let visibility_action = ServerAction::<UpdateVisibility>::new();

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
        visibility_action.version().get();
        visibility.refetch();
    });

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
            <div class="card-header">"Visibility"</div>
            <p class="text-secondary mb-4" style="font-size: 0.875rem;">
                "Control who can see this repository. Private repositories are only visible to the owner and collaborators."
            </p>

            {move || visibility_action.value().get().map(|r| match r {
                Ok(()) => view! {
                    <div class="flash flash-success">"Visibility updated."</div>
                }.into_any(),
                Err(e) => view! {
                    <div class="flash flash-error">{e.to_string()}</div>
                }.into_any(),
            })}

            <Suspense fallback=|| view! { <p class="text-secondary">"Loading..."</p> }>
                {move || {
                    let owner_val = owner();
                    let repo_val = repo();
                    Suspend::new(async move {
                        match visibility.await {
                            Ok(is_private) => view! {
                                <ActionForm action=visibility_action>
                                    <input type="hidden" name="owner" value={owner_val} />
                                    <input type="hidden" name="repo" value={repo_val} />
                                    <div class="form-group form-inline">
                                        <input type="hidden" name="is_private" value="false" />
                                        <input
                                            type="checkbox"
                                            id="is_private"
                                            name="is_private"
                                            value="true"
                                            class="form-checkbox"
                                            checked=is_private
                                        />
                                        <label for="is_private">"Private repository"</label>
                                    </div>
                                    <p class="text-secondary" style="font-size: 0.8125rem; margin-bottom: var(--space-3);">
                                        {if is_private {
                                            "This repository is currently private. Only you and collaborators can see it."
                                        } else {
                                            "This repository is currently public. Anyone can see it."
                                        }}
                                    </p>
                                    <button type="submit" class="btn btn-primary btn-sm">"Save"</button>
                                </ActionForm>
                            }.into_any(),
                            Err(e) => view! {
                                <div class="flash flash-error">{e.to_string()}</div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
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
                                {format!("{dir}/save-prompt.sh + {dir}/oxigit-context.sh + {cfg}", dir = hook_dir, cfg = config_path)}
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
