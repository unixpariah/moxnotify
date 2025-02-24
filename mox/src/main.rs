mod notify;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Notify {
        #[command(subcommand)]
        notify_cmd: NotifyCommand,
    },
}

#[derive(Subcommand)]
enum NotifyCommand {
    Focus,
    Dismiss {
        #[arg(short, long, help = "Dismiss all notifications")]
        all: bool,
        #[arg(short, long, help = "Dismiss the notification by index")]
        notification: Option<u32>,
    },
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Notify { notify_cmd }) => {
            match notify_cmd {
                NotifyCommand::Focus => notify::emit(notify::Event::Focus),
                NotifyCommand::List => notify::emit(notify::Event::List),
                NotifyCommand::Dismiss { all, notification } => {
                    notify::emit(notify::Event::Dismiss {
                        all,
                        notification: notification.unwrap_or(0),
                    })
                }
            }
            .await?
        }
        None => {
            println!("No command provided");
        }
    }

    Ok(())
}
