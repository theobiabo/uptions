use std::env;

use resend_rs::types::CreateEmailBaseOptions;
use resend_rs::{Resend, Result};

const DEFAULT_FROM_EMAIL: &str = "uptions <onboarding@uptions.xyz>";

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

    pub fn from_env() -> std::result::Result<Self, String> {
        let api_key = first_env(&["RESEND_API_KEY", "RESEND__KEY"])?;
        let default_from =
            first_env(&["RESEND_FROM_EMAIL", "RESEND__FROM_EMAIL"]).unwrap_or_else(|error| {
                tracing::warn!(
                    error = %error,
                    default_from = DEFAULT_FROM_EMAIL,
                    "using default Resend sender"
                );
                DEFAULT_FROM_EMAIL.to_owned()
            });

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

fn first_env(names: &[&str]) -> std::result::Result<String, String> {
    for name in names {
        if let Ok(value) = env::var(name) {
            return Ok(value);
        }
    }

    Err(format!("one of {} must be set", names.join(", ")))
}

pub async fn send_email(
    to: &str,
    subject: &str,
    html_body: &str,
) -> std::result::Result<(), String> {
    let client = ResendClient::from_env()?;
    client
        .send(to, subject, html_body)
        .await
        .map_err(|error| error.to_string())
}
