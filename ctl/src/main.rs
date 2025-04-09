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
    Unmute,
    Mute,
    ShowHistory,
    HideHistory,
    Inhibit,
    Uninhibit,
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
        NotifyCommand::Unmute => notify::emit(notify::Event::Unmute).await?,
        NotifyCommand::Mute => notify::emit(notify::Event::Mute).await?,
        NotifyCommand::ShowHistory => notify::emit(notify::Event::ShowHistory).await?,
        NotifyCommand::HideHistory => notify::emit(notify::Event::HideHistory).await?,
        NotifyCommand::Inhibit => notify::emit(notify::Event::Inhibit).await?,
        NotifyCommand::Uninhibit => notify::emit(notify::Event::Uninhibit).await?,
    }

    Ok(())
}
