pub use config::WebhookConfig;
pub use dispatcher::send_notification;
pub use payload::NotificationEvent;

mod config;
mod dispatcher;
mod payload;
