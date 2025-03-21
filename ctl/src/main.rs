mod notify;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: NotifyCommand,
}

#[derive(Subcommand)]
enum NotifyCommand {
    #[command(about = "Focus the notification viewer")]
    Focus,

    #[command(about = "Dismiss notifications")]
    Dismiss {
        #[arg(
            short,
            long,
            help = "Dismiss all notifications",
            conflicts_with = "notification"
        )]
        all: bool,

        #[arg(
            short,
            long,
            help = "Dismiss a specific notification by index",
            required_unless_present("all")
        )]
        notification: Option<u32>,
    },

    #[command(about = "List active notifications")]
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        NotifyCommand::Focus => notify::emit(notify::Event::Focus).await?,
        NotifyCommand::List => notify::emit(notify::Event::List).await?,
        NotifyCommand::Dismiss { all, notification } => {
            if all {
                notify::emit(notify::Event::DismissAll).await?
            } else {
                let idx = notification.expect("Clap should enforce this is present");
                notify::emit(notify::Event::DismissOne(idx)).await?
            }
        }
    }

    Ok(())
}
