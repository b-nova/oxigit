use crate::error::{OxigitError, Result};

#[derive(Clone, Debug)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub from: String,
    pub contact_email: String,
}

pub async fn send_inquiry_notification(
    config: &SmtpConfig,
    name: &str,
    email: &str,
    company: &str,
    message: &str,
) -> Result<()> {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let subject = if company.is_empty() {
        format!("New enterprise inquiry from {}", name)
    } else {
        format!("New enterprise inquiry from {} ({})", name, company)
    };

    let body = format!(
        "Name: {}\nEmail: {}\nCompany: {}\n\nMessage:\n{}",
        name, email, company, message
    );

    let email_msg = Message::builder()
        .from(
            config
                .from
                .parse()
                .map_err(|e| OxigitError::Email(format!("Invalid from address: {e}")))?,
        )
        .reply_to(
            email
                .parse()
                .map_err(|e| OxigitError::Email(format!("Invalid reply-to address: {e}")))?,
        )
        .to(config
            .contact_email
            .parse()
            .map_err(|e| OxigitError::Email(format!("Invalid contact address: {e}")))?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| OxigitError::Email(format!("Failed to build email: {e}")))?;

    let creds = Credentials::new(config.user.clone(), config.password.clone());

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
        .map_err(|e| OxigitError::Email(format!("Failed to create SMTP transport: {e}")))?
        .port(config.port)
        .credentials(creds)
        .build();

    mailer
        .send(email_msg)
        .await
        .map_err(|e| OxigitError::Email(format!("Failed to send email: {e}")))?;

    Ok(())
}
