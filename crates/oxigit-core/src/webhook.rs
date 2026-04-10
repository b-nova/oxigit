use serde::Serialize;
use tracing;

use crate::models::RepoWebhook;

#[derive(Serialize)]
pub struct WebhookPayload {
    pub event: String,
    pub repository: WebhookRepo,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub after: String,
    pub commit_message: String,
    pub callback_url: String,
}

#[derive(Serialize)]
pub struct WebhookRepo {
    pub owner: String,
    pub name: String,
}

/// Fire push webhooks asynchronously. Does not block on responses.
pub async fn fire_push_webhooks(
    webhooks: &[RepoWebhook],
    owner: &str,
    repo_name: &str,
    branch: &str,
    commit_sha: &str,
    commit_message: &str,
    callback_base_url: &str,
) {
    let payload = WebhookPayload {
        event: "push".into(),
        repository: WebhookRepo {
            owner: owner.into(),
            name: repo_name.into(),
        },
        git_ref: format!("refs/heads/{}", branch),
        after: commit_sha.into(),
        commit_message: commit_message.into(),
        callback_url: format!("{}/api/deploy-callback/{}", callback_base_url, commit_sha),
    };

    let body = match serde_json::to_string(&payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Failed to serialize webhook payload: {}", e);
            return;
        }
    };

    let client = reqwest::Client::new();

    for hook in webhooks {
        if !hook.active {
            continue;
        }

        let mut request = client
            .post(&hook.url)
            .header("Content-Type", "application/json")
            .header("X-Oxigit-Event", "push");

        // Sign with HMAC-SHA256 if secret is configured
        if let Some(ref secret) = hook.secret {
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            let mut mac =
                Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC key error");
            mac.update(body.as_bytes());
            let signature = hex::encode(mac.finalize().into_bytes());
            request = request.header("X-Oxigit-Signature", format!("sha256={}", signature));
        }

        match request.body(body.clone()).send().await {
            Ok(resp) => {
                tracing::info!("Webhook fired to {} — status {}", hook.url, resp.status());
            }
            Err(e) => {
                tracing::warn!("Webhook failed for {}: {}", hook.url, e);
            }
        }
    }
}
