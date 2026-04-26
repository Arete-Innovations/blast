
use std::env;

use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::Credentials,
        AsyncSmtpTransport,
    },
    AsyncTransport, Message, Tokio1Executor,
};

use crate::{
    cata_log,
    meltdown::{MeltDown, MeltType},
};

const DEFAULT_PORT: u16 = 587;
const DEFAULT_TLS: bool = true;

pub struct Email {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl Email {
    pub fn from_env() -> Result<Email, MeltDown> {
        let host = required_env("SMTP_HOST")?;
        let user = required_env("SMTP_USER")?;
        let pass = required_env("SMTP_PASS")?;
        let from_raw = required_env("SMTP_FROM")?;

        let port = match env::var("SMTP_PORT") {
            Ok(p) => p.parse::<u16>().map_err(|e| {
                MeltDown::new(MeltType::ConfigurationError, format!("SMTP_PORT not a u16: {}", p))
                    .with_source(e)
            })?,
            Err(_) => DEFAULT_PORT,
        };

        let tls = match env::var("SMTP_TLS") {
            Ok(v) => match v.to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                other => {
                    return Err(MeltDown::new(
                        MeltType::ConfigurationError,
                        format!("SMTP_TLS must be true/false, got `{}`", other),
                    ));
                }
            },
            Err(_) => DEFAULT_TLS,
        };

        let from: Mailbox = from_raw.parse().map_err(|e: lettre::address::AddressError| {
            MeltDown::new(MeltType::ConfigurationError, format!("SMTP_FROM not a valid mailbox: {}", from_raw))
                .with_source(e)
        })?;

        let creds = Credentials::new(user, pass);

        let builder = if tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host).map_err(|e| {
                MeltDown::new(MeltType::ConfigurationError, format!("SMTP starttls relay setup failed for {}", host))
                    .with_source(e)
            })?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host)
        };

        let transport = builder.port(port).credentials(creds).build();

        cata_log!(Info, format!("smtp transport configured: {}:{} tls={}", host, port, tls));

        Ok(Email { transport, from })
    }

    pub async fn send(
        &self,
        to: &str,
        subject: &str,
        body_text: &str,
        body_html: Option<&str>,
    ) -> Result<(), MeltDown> {
        let to_mb: Mailbox = to.parse().map_err(|e: lettre::address::AddressError| {
            MeltDown::new(MeltType::ValidationFailed, format!("invalid recipient: {}", to))
                .with_source(e)
        })?;

        let builder = Message::builder()
            .from(self.from.clone())
            .to(to_mb)
            .subject(subject);

        let message = match body_html {
            Some(html) => builder
                .multipart(
                    MultiPart::alternative()
                        .singlepart(
                            SinglePart::builder()
                                .header(ContentType::TEXT_PLAIN)
                                .body(body_text.to_string()),
                        )
                        .singlepart(
                            SinglePart::builder()
                                .header(ContentType::TEXT_HTML)
                                .body(html.to_string()),
                        ),
                )
                .map_err(|e| {
                    MeltDown::new(MeltType::ValidationFailed, "build multipart message")
                        .with_source(e)
                })?,
            None => builder
                .header(ContentType::TEXT_PLAIN)
                .body(body_text.to_string())
                .map_err(|e| {
                    MeltDown::new(MeltType::ValidationFailed, "build text message")
                        .with_source(e)
                })?,
        };

        match self.transport.send(message).await {
            Ok(_resp) => Ok(()),
            Err(e) => Err(map_smtp_error(e)),
        }
    }
}

fn required_env(key: &str) -> Result<String, MeltDown> {
    env::var(key).map_err(|_| {
        MeltDown::new(MeltType::EnvironmentError, format!("missing required env var `{}`", key))
    })
}

fn map_smtp_error(err: lettre::transport::smtp::Error) -> MeltDown {
    let is_auth = err.is_permanent() || err.is_client();
    let transient = !is_auth;
    MeltDown::new(MeltType::ExternalServiceError, format!("smtp send failed: {}", err))
        .with_source(err)
        .mark_transient(transient)
}
