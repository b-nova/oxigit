use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SshKeyInfo {
    pub id: i64,
    pub name: String,
    pub fingerprint: String,
    pub created_at: String,
}

#[server]
async fn list_ssh_keys() -> Result<Vec<SshKeyInfo>, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let keys = db::list_ssh_keys(&pool, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

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
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;

    // Parse and validate the public key, compute fingerprint
    let public_key = public_key.trim().to_string();
    let fingerprint = compute_fingerprint(&public_key)?;

    db::add_ssh_key(&pool, user.id, &name, &public_key, &fingerprint)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

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
    if !["ssh-rsa", "ssh-ed25519", "ecdsa-sha2-nistp256", "ecdsa-sha2-nistp384", "ecdsa-sha2-nistp521"]
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
    let fingerprint = format!(
        "SHA256:{}",
        base64_encode_nopad(&hash)
    );
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

#[server]
async fn delete_key(key_id: i64) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    db::delete_ssh_key(&pool, key_id, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
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
        add_action.value().get().and_then(|r| r.err().map(|e| e.to_string()))
    };

    view! {
        <div class="page-header">
            <h1 class="page-title">"Settings"</h1>
        </div>

        // Add SSH Key
        <div class="card">
            <div class="card-header">"Add SSH Key"</div>
            {move || error().map(|e| view! {
                <div class="flash flash-error">{e}</div>
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
            <Suspense fallback=|| view! { <p class="text-secondary">"Loading..."</p> }>
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
                            <div class="flash flash-error">{e.to_string()}</div>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
