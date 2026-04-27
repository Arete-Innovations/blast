
use lettre::{
    message::Mailbox,
    transport::smtp::AsyncSmtpTransport,
    Tokio1Executor,
};

pub struct Email {
    pub(crate) transport: AsyncSmtpTransport<Tokio1Executor>,
    pub(crate) from: Mailbox,
}
