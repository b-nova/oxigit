use leptos::prelude::*;

use crate::components::error_display::ErrorDisplay;

#[server]
pub async fn submit_contact_inquiry(
    name: String,
    email: String,
    company: String,
    message: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_control_pool, get_smtp_config, sfn_err};
    use oxigit_core::{db, email};

    if name.trim().is_empty() || email.trim().is_empty() || message.trim().is_empty() {
        return Err(ServerFnError::new("Name, email, and message are required"));
    }

    let pool = get_control_pool().await?;

    db::insert_contact_inquiry(
        &pool,
        name.trim(),
        email.trim(),
        company.trim(),
        message.trim(),
    )
    .await
    .map_err(sfn_err)?;

    // Send email notification if SMTP is configured
    if let Ok(Some(smtp)) = get_smtp_config().await
        && let Err(e) = email::send_inquiry_notification(
            &smtp,
            name.trim(),
            email.trim(),
            company.trim(),
            message.trim(),
        )
        .await
    {
        tracing::warn!("Failed to send contact inquiry email: {}", e);
    }

    Ok(())
}

#[component]
pub fn ContactPage() -> impl IntoView {
    let submit_action = ServerAction::<SubmitContactInquiry>::new();
    let submitted = move || matches!(submit_action.value().get(), Some(Ok(())));
    let error = move || {
        submit_action
            .value()
            .get()
            .and_then(|r| r.err().map(|e| e.to_string()))
    };

    view! {
        <div class="page-header">
            <h1 class="page-title">"Contact Us"</h1>
            <p class="page-subtitle">"Interested in Oxigit Enterprise? Tell us about your needs."</p>
        </div>

        <Show
            when=submitted
            fallback=move || {
                view! {
                    <div class="card" style="max-width: 600px;">
                        <div class="card-body">
                            {move || error().map(|e| view! {
                                <ErrorDisplay error=e />
                            })}
                            <ActionForm action=submit_action>
                                <div class="form-group">
                                    <label for="name">"Name"</label>
                                    <input type="text" id="name" name="name" required />
                                </div>
                                <div class="form-group">
                                    <label for="email">"Email"</label>
                                    <input type="email" id="email" name="email" required />
                                </div>
                                <div class="form-group">
                                    <label for="company">"Company"</label>
                                    <input type="text" id="company" name="company" />
                                </div>
                                <div class="form-group">
                                    <label for="message">"Message"</label>
                                    <textarea id="message" name="message" class="form-textarea" rows="5" required></textarea>
                                </div>
                                <button type="submit" class="btn btn-primary btn-full">
                                    "Send inquiry"
                                </button>
                            </ActionForm>
                        </div>
                    </div>
                }
            }
        >
            <div class="card" style="max-width: 600px;">
                <div class="card-body" style="text-align: center; padding: var(--space-6);">
                    <h2 style="margin-bottom: var(--space-3);">"Thank you!"</h2>
                    <p>"We've received your inquiry and will get back to you soon."</p>
                    <a href="/pricing" class="btn" style="margin-top: var(--space-4);">"Back to pricing"</a>
                </div>
            </div>
        </Show>
    }
}
