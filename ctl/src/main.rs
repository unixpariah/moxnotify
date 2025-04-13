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

        #[arg(short, long, help = "Dismiss a specific notification by index")]
        notification: Option<u32>,
    },

    #[command(about = "List active notifications")]
    List,

    #[command(about = "List active notifications")]
    Waiting,

    #[command(about = "Mute notifications")]
    Mute {
        #[command(subcommand)]
        action: SwitchAction,
    },

    #[command(about = "Manage notification history visibility")]
    History {
        #[command(subcommand)]
        action: SwitchAction,
    },

    #[command(about = "Inhibit notifications")]
    Inhibit {
        #[command(subcommand)]
        action: SwitchAction,
    },
}

#[derive(Subcommand)]
enum SwitchAction {
    On,
    Off,
    Toggle,
    State,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let event = match cli.command {
        NotifyCommand::Waiting => notify::Event::Waiting,
        NotifyCommand::Focus => notify::Event::Focus,
        NotifyCommand::List => notify::Event::List,
        NotifyCommand::Dismiss { all, notification } => {
            if all {
                notify::Event::DismissAll
            } else {
                let idx = notification.unwrap_or_default();
                notify::Event::DismissOne(idx)
            }
        }
        NotifyCommand::Mute { action } => match action {
            SwitchAction::On => notify::Event::Mute,
            SwitchAction::Off => notify::Event::Unmute,
            SwitchAction::Toggle => notify::Event::ToggleMute,
            SwitchAction::State => notify::Event::MuteState,
        },
        NotifyCommand::History { action } => match action {
            SwitchAction::On => notify::Event::ShowHistory,
            SwitchAction::Off => notify::Event::HideHistory,
            SwitchAction::Toggle => notify::Event::ToggleHistory,
            SwitchAction::State => notify::Event::HistoryState,
        },
        NotifyCommand::Inhibit { action } => match action {
            SwitchAction::On => notify::Event::Inhibit,
            SwitchAction::Off => notify::Event::Uninhibit,
            SwitchAction::Toggle => notify::Event::ToggleInhibit,
            SwitchAction::State => notify::Event::InhibitState,
        },
    };

    notify::emit(event).await.map_err(Into::into)
}
