use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

use super::GuardrailSettingsInfo;

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
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_repo_owner, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can manage collaborators"));
    }

    let collabs = db::list_collaborators_cross(&control_pool, &pool, repo_db.id)
        .await
        .map_err(sfn_err)?;

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
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can add collaborators"));
    }

    let target_user = db::get_user_by_username(&control_pool, &username)
        .await
        .map_err(|_| ServerFnError::new("User not found"))?;

    if target_user.id == repo_db.owner_id {
        return Err(ServerFnError::new("Cannot add the owner as a collaborator"));
    }

    db::add_collaborator(&pool, repo_db.id, target_user.id, "write")
        .await
        .map_err(sfn_err)?;

    Ok(())
}

#[server]
async fn remove_collaborator(
    owner: String,
    repo: String,
    user_id: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can remove collaborators"));
    }

    db::remove_collaborator(&pool, repo_db.id, user_id)
        .await
        .map_err(sfn_err)?;

    Ok(())
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
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')

# Extract last substantive prompt from transcript
PROMPT=""
if [ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ]; then
  PROMPT=$(jq -s -r '
    [.[] | select(.type == "user") | .message.content |
     if type == "string" then . else empty end]
    | map(
        if test("<command-args>") then
          (capture("<command-args>(?<p>[^<]+)</command-args>") | .p // empty)
        elif test("^<") then empty
        else .
        end
      )
    | map(select(. != null and . != "" and
        (test("^\\s*(commit|push|commit and push)\\s*$"; "i") | not)))
    | last // empty
  ' "$TRANSCRIPT" 2>/dev/null | head -c 500 || true)
fi

# Write pending metadata to .git/ (not tracked)
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo ".git")
mkdir -p "$GIT_DIR"
jq -n \
  --arg tool "{tool_name}" \
  --arg model "$MODEL" \
  --arg session_id "$SESSION_ID" \
  --arg prompt "$PROMPT" \
  '{{ tool: $tool, model: $model, session_id: $session_id, prompt: $prompt }}' \
  > "$GIT_DIR/oxigit-pending.json"

# Install prepare-commit-msg hook to inject trailers
mkdir -p "$GIT_DIR/hooks"
cat > "$GIT_DIR/hooks/prepare-commit-msg" << 'HOOKEOF'
#!/bin/bash
MSG_FILE="$1"
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo ".git")
PENDING="$GIT_DIR/oxigit-pending.json"
if [ ! -f "$PENDING" ]; then exit 0; fi
TOOL=$(jq -r '.tool // empty' "$PENDING")
MODEL=$(jq -r '.model // empty' "$PENDING")
SESSION=$(jq -r '.session_id // empty' "$PENDING")
PROMPT=$(jq -r '.prompt // empty' "$PENDING")
ARGS=()
[ -n "$TOOL" ] && ARGS+=(--trailer "Oxigit-Tool: $TOOL")
[ -n "$MODEL" ] && ARGS+=(--trailer "Oxigit-Model: $MODEL")
[ -n "$SESSION" ] && ARGS+=(--trailer "Oxigit-Session: $SESSION")
[ -n "$PROMPT" ] && ARGS+=(--trailer "Oxigit-Prompt: $PROMPT")
if [ ${{#ARGS[@]}} -gt 0 ]; then
  git interpret-trailers --in-place "${{ARGS[@]}}" "$MSG_FILE"
fi
rm -f "$PENDING"
HOOKEOF
chmod +x "$GIT_DIR/hooks/prepare-commit-msg""#,
        tool_name = tool_name,
        model_line = model_line,
    )
}

#[cfg(feature = "ssr")]
fn session_capture_script(tool_name: &str, has_model: bool) -> String {
    let model_line = if has_model {
        r#"MODEL=$(echo "$INPUT" | jq -r '.model // empty')"#
    } else {
        r#"MODEL="""#
    };
    format!(
        r#"#!/bin/bash
set -e
INPUT=$(cat)
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
{model_line}
PROMPT=$(echo "$INPUT" | jq -r '.prompt // empty')
if [ -z "$PROMPT" ]; then
  PROMPT=$(echo "$INPUT" | jq -r '.user_prompt // empty')
fi
# Keep the first prompt per session (don't overwrite with follow-ups)
if [ -f .oxigit/last-ai-session.json ]; then
  EXISTING_SESSION=$(jq -r '.session_id // empty' .oxigit/last-ai-session.json)
  if [ "$EXISTING_SESSION" = "$SESSION_ID" ]; then
    echo '{{"decision":"allow"}}' 2>/dev/null
    exit 0
  fi
fi
mkdir -p .oxigit
jq -n \
  --arg tool "{tool_name}" \
  --arg model "$MODEL" \
  --arg session_id "$SESSION_ID" \
  --arg prompt "$PROMPT" \
  '{{ tool: $tool, model: $model, session_id: $session_id, prompt: $prompt }}' \
  > .oxigit/last-ai-session.json

# Auto-install prepare-commit-msg hook if not present
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo ".git")
if [ ! -f "$GIT_DIR/hooks/prepare-commit-msg" ] && [ -f ".githooks/prepare-commit-msg" ]; then
  mkdir -p "$GIT_DIR/hooks"
  cp .githooks/prepare-commit-msg "$GIT_DIR/hooks/prepare-commit-msg"
  chmod +x "$GIT_DIR/hooks/prepare-commit-msg"
fi

# Gemini CLI requires valid JSON on stdout
echo '{{"decision":"allow"}}'
"#,
        tool_name = tool_name,
        model_line = model_line,
    )
}

#[cfg(feature = "ssr")]
fn universal_hook_script() -> &'static str {
    r#"#!/bin/bash
# Oxigit universal prepare-commit-msg hook
# Reads AI session metadata from .oxigit/last-ai-session.json
# and adds Oxigit trailers to commit messages.
#
# Setup: git config core.hooksPath .githooks

MSG_FILE="$1"

# Skip if Claude Code (it has its own richer hook system via PreToolUse)
[ -n "$CLAUDECODE" ] && exit 0

# Check for Claude Code pending file first
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo ".git")
PENDING="$GIT_DIR/oxigit-pending.json"
if [ -f "$PENDING" ]; then
  TOOL=$(jq -r '.tool // empty' "$PENDING")
  MODEL=$(jq -r '.model // empty' "$PENDING")
  SESSION=$(jq -r '.session_id // empty' "$PENDING")
  PROMPT=$(jq -r '.prompt // empty' "$PENDING")
  ARGS=()
  [ -n "$TOOL" ] && ARGS+=(--trailer "Oxigit-Tool: $TOOL")
  [ -n "$MODEL" ] && ARGS+=(--trailer "Oxigit-Model: $MODEL")
  [ -n "$SESSION" ] && ARGS+=(--trailer "Oxigit-Session: $SESSION")
  [ -n "$PROMPT" ] && ARGS+=(--trailer "Oxigit-Prompt: $PROMPT")
  if [ ${#ARGS[@]} -gt 0 ]; then
    git interpret-trailers --in-place "${ARGS[@]}" "$MSG_FILE"
  fi
  rm -f "$PENDING"
  exit 0
fi

# Check for session metadata from Codex/Gemini/other tools
SESSION_FILE=".oxigit/last-ai-session.json"
if [ -f "$SESSION_FILE" ]; then
  # Only use if written recently (within last 30 minutes)
  if [ "$(find "$SESSION_FILE" -mmin -30 2>/dev/null)" ]; then
    TOOL=$(jq -r '.tool // empty' "$SESSION_FILE")
    MODEL=$(jq -r '.model // empty' "$SESSION_FILE")
    SESSION=$(jq -r '.session_id // empty' "$SESSION_FILE")
    PROMPT=$(jq -r '.prompt // empty' "$SESSION_FILE")
    ARGS=()
    [ -n "$TOOL" ] && ARGS+=(--trailer "Oxigit-Tool: $TOOL")
    [ -n "$MODEL" ] && ARGS+=(--trailer "Oxigit-Model: $MODEL")
    [ -n "$SESSION" ] && ARGS+=(--trailer "Oxigit-Session: $SESSION")
    [ -n "$PROMPT" ] && ARGS+=(--trailer "Oxigit-Prompt: $PROMPT")
    if [ ${#ARGS[@]} -gt 0 ]; then
      git interpret-trailers --in-place "${ARGS[@]}" "$MSG_FILE"
    fi
  fi
fi
"#
}

#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
struct AiToolConfig {
    tool_id: &'static str,
    display_name: &'static str,
    hook_dir: &'static str,
    config_path: &'static str,
    has_model: bool,
    /// If true, uses PreToolUse on git commit (Claude Code).
    /// If false, uses UserPromptSubmit to capture session + .githooks for trailers (Codex/Gemini).
    uses_pre_tool: bool,
    tool_event: &'static str,
    tool_matcher: &'static str,
    prompt_event: &'static str,
}

#[cfg(feature = "ssr")]
fn config_json(tool: &AiToolConfig) -> String {
    if tool.uses_pre_tool {
        // Claude Code: PreToolUse hook on git commit
        format!(
            r#"{{
  "hooks": {{
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
            tool_event = tool.tool_event,
            tool_matcher = tool.tool_matcher,
            hook_dir = tool.hook_dir,
        )
    } else {
        // Codex/Gemini: capture session on prompt submit
        format!(
            r#"{{
  "hooks": {{
    "{prompt_event}": [
      {{
        "matcher": "",
        "hooks": [
          {{
            "type": "command",
            "command": "{hook_dir}/oxigit-session.sh",
            "timeout": 5000
          }}
        ]
      }}
    ]
  }}
}}"#,
            prompt_event = tool.prompt_event,
            hook_dir = tool.hook_dir,
        )
    }
}

const AI_TOOLS: &[AiToolConfig] = &[
    AiToolConfig {
        tool_id: "claude-code",
        display_name: "Claude Code",
        hook_dir: ".claude/hooks",
        config_path: ".claude/settings.json",
        has_model: true,
        uses_pre_tool: true,
        tool_event: "PreToolUse",
        tool_matcher: "Bash",
        prompt_event: "",
    },
    AiToolConfig {
        tool_id: "codex",
        display_name: "Codex CLI",
        hook_dir: ".codex/hooks",
        config_path: ".codex/hooks.json",
        has_model: true,
        uses_pre_tool: false,
        tool_event: "",
        tool_matcher: "",
        prompt_event: "UserPromptSubmit",
    },
    AiToolConfig {
        tool_id: "gemini-cli",
        display_name: "Gemini CLI",
        hook_dir: ".gemini/hooks",
        config_path: ".gemini/settings.json",
        has_model: false,
        uses_pre_tool: false,
        tool_event: "",
        tool_matcher: "",
        prompt_event: "BeforeAgent",
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
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_path, get_repo_pools};
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can install AI hooks"));
    }

    let tool = get_tool_config(&tool_id)
        .ok_or_else(|| ServerFnError::new("Unknown AI tool"))?;

    let repo_path = get_repo_path(&owner, &repo).await?;

    let branch = git::default_branch(&repo_path)
        .map_err(sfn_err)?
        .ok_or_else(|| ServerFnError::new("Repository has no branches"))?;

    let config = config_json(tool);

    let files = if tool.uses_pre_tool {
        // Claude Code: PreToolUse hook on git commit
        let context_script = hook_script(tool.tool_id, tool.has_model);
        let context_path = format!("{}/oxigit-context.sh", tool.hook_dir);
        vec![
            (context_path, context_script, true),
            (tool.config_path.to_string(), config, false),
        ]
    } else {
        // Codex/Gemini: session capture + universal prepare-commit-msg
        let session_script = session_capture_script(tool.tool_id, tool.has_model);
        let session_path = format!("{}/oxigit-session.sh", tool.hook_dir);
        let universal = universal_hook_script().to_string();
        vec![
            (session_path, session_script, true),
            (tool.config_path.to_string(), config, false),
            (".githooks/prepare-commit-msg".to_string(), universal, true),
        ]
    };

    let files_refs: Vec<(&str, &str, bool)> = files.iter()
        .map(|(p, c, e)| (p.as_str(), c.as_str(), *e))
        .collect();

    git::add_files_to_branch(
        &repo_path,
        &branch,
        &files_refs,
        &format!("chore: install {} AI hook for Oxigit", tool.display_name),
        &user.username,
        &format!("{}@oxigit", user.username),
    )
    .map_err(sfn_err)?;

    Ok(())
}

#[server]
async fn check_installed_hooks(
    owner: String,
    repo: String,
) -> Result<Vec<HookStatus>, ServerFnError> {
    use crate::server_fns::get_repo_path;
    use oxigit_core::git;

    let repo_path = get_repo_path(&owner, &repo).await?;

    let branch = match git::default_branch(&repo_path) {
        Ok(Some(b)) => b,
        _ => return Ok(vec![]),
    };

    let mut statuses = Vec::new();
    for tool in AI_TOOLS {
        let script_path = if tool.uses_pre_tool {
            format!("{}/oxigit-context.sh", tool.hook_dir)
        } else {
            format!("{}/oxigit-session.sh", tool.hook_dir)
        };

        let script_installed = git::read_blob(&repo_path, &branch, &script_path).ok();

        if script_installed.is_some() {
            let expected_script = if tool.uses_pre_tool {
                hook_script(tool.tool_id, tool.has_model)
            } else {
                session_capture_script(tool.tool_id, tool.has_model)
            };
            let script_ok = script_installed
                .map(|b| String::from_utf8_lossy(&b).as_ref() == expected_script)
                .unwrap_or(false);
            let config_ok = git::read_blob(&repo_path, &branch, tool.config_path)
                .map(|b| String::from_utf8_lossy(&b).as_ref() == config_json(tool))
                .unwrap_or(false);

            statuses.push(HookStatus {
                tool_id: tool.tool_id.to_string(),
                up_to_date: script_ok && config_ok,
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
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;
    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Not authorized"));
    }

    let hooks = db::list_webhooks(&pool, repo_db.id)
        .await
        .map_err(sfn_err)?;
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
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;
    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Not authorized"));
    }

    let secret = if secret.is_empty() { None } else { Some(secret) };
    db::create_webhook(&pool, repo_db.id, &url, secret.as_deref())
        .await
        .map_err(sfn_err)?;
    Ok(())
}

#[server]
async fn fetch_repo_visibility(
    owner: String,
    repo: String,
) -> Result<bool, ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

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
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can change visibility"));
    }

    db::update_repository_visibility(&pool, repo_db.id, is_private)
        .await
        .map_err(sfn_err)?;

    Ok(())
}

#[server]
async fn delete_webhook(
    owner: String,
    repo: String,
    webhook_id: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;
    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Not authorized"));
    }

    db::delete_webhook(&pool, webhook_id, repo_db.id)
        .await
        .map_err(sfn_err)?;
    Ok(())
}

#[server]
async fn get_guardrail_settings(
    owner: String,
    repo: String,
) -> Result<GuardrailSettingsInfo, ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools, get_user_entitlements};
    use oxigit_core::db;
    use super::GuardrailRuleInfo;

    let user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let entitlements = get_user_entitlements(user.id).await?;
    if !entitlements.team_features {
        return Err(ServerFnError::new(
            "Guardrails require a Team plan. Upgrade at /pricing",
        ));
    }

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await.map_err(sfn_err)?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can manage guardrails"));
    }

    let rules = db::get_guardrail_rules(&pool, repo_db.id)
        .await.map_err(sfn_err)?;
    let config = db::get_guardrail_config(&pool, repo_db.id)
        .await.map_err(sfn_err)?;

    let categories = ["security", "breaking", "performance", "quality"];
    let rule_infos: Vec<GuardrailRuleInfo> = categories.iter().map(|cat| {
        let action = rules.iter()
            .find(|r| r.category == *cat)
            .map(|r| r.action.clone())
            .unwrap_or_else(|| "off".to_string());
        GuardrailRuleInfo { category: cat.to_string(), action }
    }).collect();

    Ok(GuardrailSettingsInfo {
        rules: rule_infos,
        min_vibe_score: config.as_ref().and_then(|c| c.min_vibe_score),
        max_files_per_push: config.as_ref().and_then(|c| c.max_files_per_push),
    })
}

#[server]
async fn save_guardrail_settings(
    owner: String,
    repo: String,
    security: String,
    breaking: String,
    performance: String,
    quality: String,
    min_vibe_score: Option<i64>,
    max_files_per_push: Option<i64>,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_path, get_repo_pools, get_user_entitlements};
    use oxigit_core::db;

    let user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let entitlements = get_user_entitlements(user.id).await?;
    if !entitlements.team_features {
        return Err(ServerFnError::new(
            "Guardrails require a Team plan. Upgrade at /pricing",
        ));
    }

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await.map_err(sfn_err)?;

    if repo_db.owner_id != user.id {
        return Err(ServerFnError::new("Only the owner can manage guardrails"));
    }

    // Save rules
    for (cat, action) in [("security", &security), ("breaking", &breaking), ("performance", &performance), ("quality", &quality)] {
        db::upsert_guardrail_rule(&pool, repo_db.id, cat, action)
            .await.map_err(sfn_err)?;
    }

    // Save config
    db::upsert_guardrail_config(&pool, repo_db.id, min_vibe_score, max_files_per_push)
        .await.map_err(sfn_err)?;

    // Manage pre-receive hook based on whether any block rules exist
    let has_block = [&security, &breaking, &performance, &quality].iter().any(|a| a.as_str() == "block");
    let repo_path = get_repo_path(&owner, &repo).await?;
    let hook_path = repo_path.join("hooks").join("pre-receive");

    if has_block {
        // Write pre-receive hook
        let _ = std::fs::create_dir_all(repo_path.join("hooks"));
        let script = generate_pre_receive_hook(repo_db.id);
        let _ = std::fs::write(&hook_path, script);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755));
        }
    } else {
        // Remove pre-receive hook if it exists
        let _ = std::fs::remove_file(&hook_path);
    }

    leptos_axum::redirect(&format!("/{}/{}/settings", owner, repo));
    Ok(())
}

#[cfg(feature = "ssr")]
fn generate_pre_receive_hook(repo_id: i64) -> String {
    format!(r#"#!/bin/bash
# Oxigit AI Guardrail pre-receive hook
while read old new ref; do
  if [ "$old" = "0000000000000000000000000000000000000000" ]; then
    DIFF=$(git diff-tree -p --root "$new" 2>/dev/null)
  else
    DIFF=$(git diff "$old" "$new" 2>/dev/null)
  fi
  FILES=$(git diff-tree --no-commit-id --name-only -r "$old" "$new" 2>/dev/null | wc -l)
  RESULT=$(curl -sf -X POST "http://127.0.0.1:${{OXIGIT_PORT}}/internal/guardrail-check" \
    -H "X-Internal-Secret: ${{OXIGIT_SECRET}}" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    --data-urlencode "repo_id={repo_id}" \
    --data-urlencode "old=$old" \
    --data-urlencode "new=$new" \
    --data-urlencode "ref=$ref" \
    --data-urlencode "file_count=$FILES" \
    --data-urlencode "diff=$DIFF" 2>/dev/null)
  if [ $? -ne 0 ] || echo "$RESULT" | head -1 | grep -q "^BLOCKED"; then
    echo "" >&2
    echo "$RESULT" >&2
    echo "" >&2
    exit 1
  fi
done
"#)
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
    let save_guardrails_action = ServerAction::<SaveGuardrailSettings>::new();

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
                    <ErrorDisplay error=e.to_string() />
                }.into_any(),
            })}

            <Suspense fallback=|| view! { <LoadingPage /> }>
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
                                <ErrorDisplay error=e.to_string() />
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
                <ErrorDisplay error=e />
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

            <Suspense fallback=|| view! { <LoadingPage /> }>
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
                                <ErrorDisplay error=e.to_string() />
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
                <ErrorDisplay error=e />
            })}
            {move || hook_success().then(|| view! {
                <div class="flash flash-success">"Hook installed. Pull to get the new files."</div>
            })}

            // Claude Code (native hook)
            {AI_TOOLS.iter().map(|tool| {
                let tool_id = tool.tool_id.to_string();
                let tool_id_check = tool.tool_id.to_string();
                let display_name = tool.display_name.to_string();
                let script_name = if tool.uses_pre_tool { "oxigit-context.sh" } else { "oxigit-session.sh" };
                let hook_dir = tool.hook_dir.to_string();
                let config_path = tool.config_path.to_string();
                let hook_status = Signal::derive(move || {
                    installed_hooks.get()
                        .and_then(|r| r.ok())
                        .and_then(|statuses| {
                            statuses.iter()
                                .find(|s| s.tool_id == tool_id_check.clone())
                                .map(|s| s.up_to_date)
                        })
                });
                view! {
                    <div class="list-item">
                        <div>
                            <span class="font-semibold">{display_name}</span>
                            <span class="text-secondary" style="font-size: 0.8125rem; margin-left: 0.5rem;">
                                {format!("{dir}/{script} + {cfg}", dir = hook_dir, script = script_name, cfg = config_path)}
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

            <p class="text-tertiary" style="font-size: 0.75rem; padding: var(--space-2) var(--space-4) var(--space-3);">
                "Codex CLI requires " <code>"codex_hooks = true"</code> " in "
                <code>"~/.codex/config.toml"</code> " under " <code>"[features]"</code> "."
            </p>
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
                                <ErrorDisplay error=e.to_string() />
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>

        // --- AI Guardrails Section ---
        <div class="card mb-4">
            <div class="card-header">"AI Guardrails"</div>
            <div style="padding: var(--space-4);">
                <p class="text-secondary mb-3" style="font-size: 0.8125rem;">
                    "Configure rules to scan AI-generated code on push. "
                    <strong>"Warn"</strong> " logs violations. "
                    <strong>"Block"</strong> " rejects the push."
                </p>
                <Suspense fallback=|| view! { <LoadingPage /> }>
                    {move || {
                        let on = owner();
                        let rn = repo();
                        Suspend::new(async move {
                            match get_guardrail_settings(on.clone(), rn.clone()).await {
                                Ok(settings) => {
                                    let sec = settings.rules.iter().find(|r| r.category == "security").map(|r| r.action.clone()).unwrap_or("off".into());
                                    let brk = settings.rules.iter().find(|r| r.category == "breaking").map(|r| r.action.clone()).unwrap_or("off".into());
                                    let perf = settings.rules.iter().find(|r| r.category == "performance").map(|r| r.action.clone()).unwrap_or("off".into());
                                    let qual = settings.rules.iter().find(|r| r.category == "quality").map(|r| r.action.clone()).unwrap_or("off".into());

                                    view! {
                                        <ActionForm action=save_guardrails_action>
                                            <input type="hidden" name="owner" value={on} />
                                            <input type="hidden" name="repo" value={rn} />

                                            <div class="guardrail-grid">
                                                <div class="guardrail-row">
                                                    <label class="guardrail-label">"Security"</label>
                                                    <span class="text-tertiary" style="font-size: 0.75rem;">"Secrets, SQL injection, eval"</span>
                                                    <select name="security" class="form-input form-input-sm">
                                                        <option value="off" selected={sec == "off"}>"Off"</option>
                                                        <option value="warn" selected={sec == "warn"}>"Warn"</option>
                                                        <option value="block" selected={sec == "block"}>"Block"</option>
                                                    </select>
                                                </div>
                                                <div class="guardrail-row">
                                                    <label class="guardrail-label">"Breaking"</label>
                                                    <span class="text-tertiary" style="font-size: 0.75rem;">"Removed public APIs"</span>
                                                    <select name="breaking" class="form-input form-input-sm">
                                                        <option value="off" selected={brk == "off"}>"Off"</option>
                                                        <option value="warn" selected={brk == "warn"}>"Warn"</option>
                                                        <option value="block" selected={brk == "block"}>"Block"</option>
                                                    </select>
                                                </div>
                                                <div class="guardrail-row">
                                                    <label class="guardrail-label">"Performance"</label>
                                                    <span class="text-tertiary" style="font-size: 0.75rem;">"Binary files"</span>
                                                    <select name="performance" class="form-input form-input-sm">
                                                        <option value="off" selected={perf == "off"}>"Off"</option>
                                                        <option value="warn" selected={perf == "warn"}>"Warn"</option>
                                                        <option value="block" selected={perf == "block"}>"Block"</option>
                                                    </select>
                                                </div>
                                                <div class="guardrail-row">
                                                    <label class="guardrail-label">"Quality"</label>
                                                    <span class="text-tertiary" style="font-size: 0.75rem;">"TODO/FIXME/HACK"</span>
                                                    <select name="quality" class="form-input form-input-sm">
                                                        <option value="off" selected={qual == "off"}>"Off"</option>
                                                        <option value="warn" selected={qual == "warn"}>"Warn"</option>
                                                        <option value="block" selected={qual == "block"}>"Block"</option>
                                                    </select>
                                                </div>
                                            </div>

                                            <div class="form-group mt-3">
                                                <label>"Max files per push"</label>
                                                <input type="number" name="max_files_per_push" class="form-input form-input-sm"
                                                    style="width: 100px;"
                                                    placeholder="No limit"
                                                    value={settings.max_files_per_push.map(|v| v.to_string()).unwrap_or_default()} />
                                            </div>

                                            <button type="submit" class="btn btn-primary btn-sm mt-3">"Save Guardrails"</button>
                                        </ActionForm>
                                    }.into_any()
                                }
                                Err(e) => view! {
                                    <ErrorDisplay error=e.to_string() />
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </div>
    }
}
