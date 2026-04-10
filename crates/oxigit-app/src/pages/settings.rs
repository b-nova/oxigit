use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SshKeyInfo {
    pub id: i64,
    pub name: String,
    pub fingerprint: String,
    pub created_at: String,
}

#[server]
async fn list_ssh_keys() -> Result<Vec<SshKeyInfo>, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, sfn_err};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_control_pool().await?;
    let keys = db::list_ssh_keys(&pool, user.id).await.map_err(sfn_err)?;

    Ok(keys
        .into_iter()
        .map(|k| SshKeyInfo {
            id: k.id,
            name: k.name,
            fingerprint: k.fingerprint,
            created_at: k.created_at,
        })
        .collect())
}

#[server]
async fn add_ssh_key(name: String, public_key: String) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, sfn_err};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_control_pool().await?;

    // Parse and validate the public key, compute fingerprint
    let public_key = public_key.trim().to_string();
    let fingerprint = compute_fingerprint(&public_key)?;

    db::add_ssh_key(&pool, user.id, &name, &public_key, &fingerprint)
        .await
        .map_err(sfn_err)?;

    Ok(())
}

#[cfg(feature = "ssr")]
fn compute_fingerprint(public_key: &str) -> Result<String, ServerFnError> {
    let parts: Vec<&str> = public_key.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(ServerFnError::new(
            "Invalid SSH key format. Expected: ssh-<type> <base64-data> [comment]",
        ));
    }

    let key_type = parts[0];
    if ![
        "ssh-rsa",
        "ssh-ed25519",
        "ecdsa-sha2-nistp256",
        "ecdsa-sha2-nistp384",
        "ecdsa-sha2-nistp521",
    ]
    .contains(&key_type)
    {
        return Err(ServerFnError::new(format!(
            "Unsupported key type: {}",
            key_type
        )));
    }

    use sha2::{Digest, Sha256};
    let key_data = base64_decode_key(parts[1])?;
    let hash = Sha256::digest(&key_data);
    let fingerprint = format!("SHA256:{}", base64_encode_nopad(&hash));
    Ok(fingerprint)
}

#[cfg(feature = "ssr")]
fn base64_decode_key(input: &str) -> Result<Vec<u8>, ServerFnError> {
    let mut output = Vec::new();
    let chars: Vec<u8> = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .filter_map(|c| match c {
            'A'..='Z' => Some(c as u8 - b'A'),
            'a'..='z' => Some(c as u8 - b'a' + 26),
            '0'..='9' => Some(c as u8 - b'0' + 52),
            '+' => Some(62),
            '/' => Some(63),
            '=' => None,
            _ => None,
        })
        .collect();

    for chunk in chars.chunks(4) {
        if chunk.len() >= 2 {
            output.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            output.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() >= 4 {
            output.push((chunk[2] << 6) | chunk[3]);
        }
    }
    Ok(output)
}

#[cfg(feature = "ssr")]
fn base64_encode_nopad(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[(n & 63) as usize] as char);
        }
    }
    result
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmSettingsInfo {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

#[server]
async fn fetch_llm_settings() -> Result<LlmSettingsInfo, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, get_llm_config, sfn_err};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_control_pool().await?;
    let (default_provider, default_key, default_model, default_base_url) = get_llm_config().await?;

    let settings = db::get_user_settings(&pool, user.id)
        .await
        .map_err(sfn_err)?;

    Ok(LlmSettingsInfo {
        provider: settings
            .as_ref()
            .and_then(|s| s.llm_provider.clone())
            .unwrap_or(default_provider),
        api_key: settings
            .as_ref()
            .and_then(|s| s.llm_api_key.clone())
            .unwrap_or_else(|| default_key.unwrap_or_default()),
        model: settings
            .as_ref()
            .and_then(|s| s.llm_model.clone())
            .unwrap_or(default_model),
        base_url: settings
            .as_ref()
            .and_then(|s| s.llm_base_url.clone())
            .unwrap_or_else(|| default_base_url.unwrap_or_default()),
    })
}

#[server]
async fn save_llm_settings(
    provider: String,
    api_key: String,
    model: String,
    base_url: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, sfn_err};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_control_pool().await?;

    let provider = if provider.is_empty() || provider == "none" {
        None
    } else {
        Some(provider)
    };
    let api_key = if api_key.is_empty() {
        None
    } else {
        Some(api_key)
    };
    let model = if model.is_empty() { None } else { Some(model) };
    let base_url = if base_url.is_empty() {
        None
    } else {
        Some(base_url)
    };

    db::upsert_user_settings(
        &pool,
        user.id,
        provider.as_deref(),
        api_key.as_deref(),
        model.as_deref(),
        base_url.as_deref(),
    )
    .await
    .map_err(sfn_err)?;

    Ok(())
}

#[server]
async fn delete_key(key_id: i64) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, sfn_err};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_control_pool().await?;
    db::delete_ssh_key(&pool, key_id, user.id)
        .await
        .map_err(sfn_err)?;
    Ok(())
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    let keys = Resource::new(|| (), |_| list_ssh_keys());
    let add_action = ServerAction::<AddSshKey>::new();
    let delete_action = ServerAction::<DeleteKey>::new();

    // Refetch keys when add or delete completes
    Effect::new(move || {
        add_action.version().get();
        delete_action.version().get();
        keys.refetch();
    });

    let error = move || {
        add_action
            .value()
            .get()
            .and_then(|r| r.err().map(|e| e.to_string()))
    };

    // LLM settings
    let llm_settings = Resource::new(|| (), |_| fetch_llm_settings());
    let save_llm_action = ServerAction::<SaveLlmSettings>::new();

    let llm_save_success = move || {
        save_llm_action
            .value()
            .get()
            .and_then(|r| r.ok())
            .map(|_| true)
    };
    let llm_save_error = move || {
        save_llm_action
            .value()
            .get()
            .and_then(|r| r.err().map(|e| e.to_string()))
    };

    view! {
        <div class="page-header">
            <h1 class="page-title">"Settings"</h1>
        </div>

        // LLM Configuration
        <div class="card mb-4">
            <div class="card-header">"AI / LLM Configuration"</div>
            {move || llm_save_success().map(|_| view! {
                <div class="flash flash-success">"LLM settings saved."</div>
            })}
            {move || llm_save_error().map(|e| view! {
                <ErrorDisplay error=e />
            })}
            <Suspense fallback=|| view! { <LoadingPage /> }>
                {move || Suspend::new(async move {
                    let defaults = llm_settings.await.unwrap_or(LlmSettingsInfo {
                        provider: "ollama".into(),
                        api_key: String::new(),
                        model: "qwen3-coder".into(),
                        base_url: String::new(),
                    });
                    let default_provider = defaults.provider.clone();
                    let default_api_key = defaults.api_key.clone();
                    let default_model = defaults.model.clone();
                    let default_base_url = defaults.base_url.clone();
                    let (selected_provider, set_selected_provider) = signal(default_provider.clone());
                    let (model_value, set_model_value) = signal(default_model.clone());
                    let needs_api_key = move || {
                        let p = selected_provider.get();
                        p == "openai" || p == "anthropic"
                    };
                    let model_placeholder = move || {
                        match selected_provider.get().as_str() {
                            "openai" => "gpt-4o-mini",
                            "anthropic" => "claude-haiku-4-5-20251001",
                            _ => "qwen3-coder",
                        }.to_string()
                    };
                    let default_model_for_provider = move |provider: &str| -> String {
                        match provider {
                            "openai" => "gpt-4o-mini".to_string(),
                            "anthropic" => "claude-haiku-4-5-20251001".to_string(),
                            _ => "qwen3-coder".to_string(),
                        }
                    };
                    view! {
                        <p class="text-secondary" style="margin-bottom: var(--space-3);">
                            "Oxigit uses an LLM to generate plain-English summaries of commits and pull requests. "
                            "By default, it connects to a local "
                            <a href="https://ollama.com" target="_blank">"Ollama"</a>
                            " instance running the "
                            <code>"qwen3-coder"</code>
                            " model — free and private, no API key required. "
                            "You can also bring your own cloud provider below."
                        </p>
                        <ActionForm action=save_llm_action>
                            <div class="form-group">
                                <label for="provider">"Provider"</label>
                                <select
                                    id="provider"
                                    name="provider"
                                    class="form-select"
                                    on:change=move |ev| {
                                        let target = event_target::<leptos::web_sys::HtmlSelectElement>(&ev);
                                        let new_provider = target.value();
                                        set_model_value.set(default_model_for_provider(&new_provider));
                                        set_selected_provider.set(new_provider);
                                    }
                                >
                                    <option value="ollama" selected={default_provider == "ollama"}>"Ollama (default, no API key needed)"</option>
                                    <option value="openai" selected={default_provider == "openai"}>"OpenAI"</option>
                                    <option value="anthropic" selected={default_provider == "anthropic"}>"Anthropic"</option>
                                    <option value="none" selected={default_provider == "none"}>"None (disabled)"</option>
                                </select>
                            </div>
                            <div class="form-group" style:display=move || if needs_api_key() { "block" } else { "none" }>
                                <label for="api_key">"API Key"</label>
                                <input
                                    type="password"
                                    id="api_key"
                                    name="api_key"
                                    value={default_api_key}
                                    placeholder="sk-... or your API key"
                                />
                            </div>
                            <div class="form-group">
                                <label for="model">"Model"</label>
                                <input
                                    type="text"
                                    id="model"
                                    name="model"
                                    prop:value=model_value
                                    placeholder=model_placeholder
                                    on:input=move |ev| {
                                        let target = event_target::<leptos::web_sys::HtmlInputElement>(&ev);
                                        set_model_value.set(target.value());
                                    }
                                />
                            </div>
                            <div class="form-group">
                                <label for="base_url">"Base URL"</label>
                                <input
                                    type="text"
                                    id="base_url"
                                    name="base_url"
                                    value={default_base_url}
                                    placeholder="For Ollama or custom endpoints (optional)"
                                />
                            </div>
                            <button type="submit" class="btn btn-primary">"Save"</button>
                        </ActionForm>
                    }.into_any()
                })}
            </Suspense>
        </div>

        // Add SSH Key
        <div class="card">
            <div class="card-header">"Add SSH Key"</div>
            {move || error().map(|e| view! {
                <ErrorDisplay error=e />
            })}
            <ActionForm action=add_action>
                <div class="form-group">
                    <label for="name">"Key name"</label>
                    <input type="text" id="name" name="name" required placeholder="e.g. My Laptop" />
                </div>
                <div class="form-group">
                    <label for="public_key">"Public key"</label>
                    <textarea
                        id="public_key"
                        name="public_key"
                        required
                        rows="3"
                        placeholder="ssh-ed25519 AAAA... user@host"
                        class="form-textarea form-textarea-mono"
                    ></textarea>
                </div>
                <button type="submit" class="btn btn-primary">"Add key"</button>
            </ActionForm>
        </div>

        // Existing keys
        <div class="card">
            <div class="card-header">"SSH Keys"</div>
            <Suspense fallback=|| view! { <LoadingPage /> }>
                {move || Suspend::new(async move {
                    match keys.await {
                        Ok(keys) if keys.is_empty() => view! {
                            <p class="text-secondary" style="padding: var(--space-4) 0;">"No SSH keys added yet."</p>
                        }.into_any(),
                        Ok(keys) => view! {
                            <ul class="list">
                                {keys.into_iter().map(|key| {
                                    let name = key.name.clone();
                                    let fingerprint = key.fingerprint.clone();
                                    let created = key.created_at.clone();
                                    let key_id = key.id;
                                    view! {
                                        <li class="list-item">
                                            <div>
                                                <div class="font-semibold">{name}</div>
                                                <code class="font-mono text-secondary" style="font-size: 0.75rem;">{fingerprint}</code>
                                                <div class="list-item-meta">"Added " {created}</div>
                                            </div>
                                            <ActionForm action=delete_action>
                                                <input type="hidden" name="key_id" value={key_id.to_string()} />
                                                <button type="submit" class="btn btn-danger btn-sm">"Delete"</button>
                                            </ActionForm>
                                        </li>
                                    }
                                }).collect::<Vec<_>>()}
                            </ul>
                        }.into_any(),
                        Err(e) => view! {
                            <ErrorDisplay error=e.to_string() />
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
