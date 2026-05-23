use super::{App, AppAction};
use crate::cli::chat::commands::backend::CommandResponse;

/// Apply a [`CommandResponse`] to the given [`App`], updating conversation
/// state and returning the appropriate [`AppAction`].
pub fn apply_response(app: &mut App, resp: CommandResponse) -> AppAction {
    match resp {
        CommandResponse::Text(t) | CommandResponse::Markdown(t) => {
            let _ = app.session.conversation.append_assistant(&t);
            AppAction::Redraw
        }
        CommandResponse::Ok => AppAction::Redraw,
        CommandResponse::NotWired { reason } => {
            let _ = app
                .session
                .conversation
                .append_assistant(&format!("not wired: {}", reason));
            AppAction::Redraw
        }
        CommandResponse::Error(e) => {
            let _ = app
                .session
                .conversation
                .append_assistant(&format!("error: {}", e));
            AppAction::Redraw
        }
        CommandResponse::EffectExit => {
            app.confirm_quit = true;
            AppAction::Redraw
        }
        CommandResponse::EffectClearView => AppAction::Redraw,
        CommandResponse::EffectThemeDark => {
            app.session.meta.theme = "dark".to_string();
            let _ = app.save_meta();
            AppAction::Redraw
        }
        CommandResponse::EffectThemeLight => {
            app.session.meta.theme = "light".to_string();
            let _ = app.save_meta();
            AppAction::Redraw
        }
        CommandResponse::EffectStartInjectInline => AppAction::Redraw,
        CommandResponse::EffectStartNewSession => {
            let _ = app
                .session
                .conversation
                .append_assistant("Starting new session...");
            AppAction::Redraw
        }
    }
}
