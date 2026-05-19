pub use config::WebhookConfig;
pub use dispatcher::send_notification;
pub use payload::NotificationEvent;
pub use transport::{MockWebhookTransport, ReqwestWebhookTransport, WebhookTransport};

mod config;
mod dispatcher;
mod payload;
mod transport;
