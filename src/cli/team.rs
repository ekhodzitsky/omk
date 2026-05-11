use anyhow::Result;
use tokio_util::sync::CancellationToken;

mod args;
mod inspect;
mod manage;
mod proof;
mod run;
mod run_support;

pub(crate) use args::{Args, TeamCommands};

pub(crate) async fn run(args: Args, cancel: CancellationToken) -> Result<()> {
    match args.command {
        TeamCommands::Run(args) => run::run_team(args, cancel).await,
        TeamCommands::List => inspect::list_teams().await,
        TeamCommands::Status(args) => inspect::status(args).await,
        TeamCommands::Rename(args) => manage::rename_team(args).await,
        TeamCommands::Export(args) => manage::export_team(args).await,
        TeamCommands::Import(args) => manage::import_team(args).await,
        TeamCommands::Shutdown(args) => manage::shutdown(args).await,
        TeamCommands::Health(args) => inspect::health(args).await,
        TeamCommands::Cleanup(args) => manage::cleanup(args).await,
        TeamCommands::Roles => args::roles(),
    }
}
