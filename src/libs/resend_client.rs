use std::env;

use resend_rs::types::CreateEmailBaseOptions;
use resend_rs::{Resend, Result};

pub struct ResendClient {
    resend: Resend,
    default_from: String,
}

impl ResendClient {
    pub fn new(api_key: &str, default_from: &str) -> Self {
        let resend = Resend::new(api_key);
        Self {
            resend,
            default_from: default_from.to_string(),
        }
    }

    pub fn from_env() -> std::result::Result<Self, env::VarError> {
        let api_key = env::var("RESEND_API_KEY")?;
        let default_from = env::var("RESEND_FROM_EMAIL")?;

        Ok(Self::new(&api_key, &default_from))
    }

    fn configure_email(
        &self,
        from: &str,
        to: &str,
        subject: &str,
        html_body: &str,
    ) -> CreateEmailBaseOptions {
        CreateEmailBaseOptions::new(from, [to], subject).with_html(html_body)
    }

    pub async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<()> {
        let email = self.configure_email(&self.default_from, to, subject, html_body);
        self.resend.emails.send(email).await?;
        Ok(())
    }

    pub async fn send_with_from(
        &self,
        from: &str,
        to: &str,
        subject: &str,
        html_body: &str,
    ) -> Result<()> {
        let email = self.configure_email(from, to, subject, html_body);
        self.resend.emails.send(email).await?;
        Ok(())
    }
}

pub async fn send_email(
    to: &str,
    subject: &str,
    html_body: &str,
) -> std::result::Result<(), String> {
    let client = ResendClient::from_env().map_err(|error| error.to_string())?;
    client
        .send(to, subject, html_body)
        .await
        .map_err(|error| error.to_string())
}
