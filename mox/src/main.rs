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
}

pub enum Event {
    Focus,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Notify { notify_cmd }) => match notify_cmd {
            NotifyCommand::Focus => {
                notify::emit(Event::Focus).await?;
            }
        },
        None => {
            println!("No command provided");
        }
    }

    Ok(())
}
