use std::fs;
use std::path::PathBuf;
use std::{env, path::absolute};

use clap::{Parser, Subcommand};

use crate::board::find_circuitpy;
use crate::workspace::ensure_workspace_dir;

/// Main CLI entry struct
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Main CLI command options
#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    Server(ServerCommand),
    Ping,
    Echo {
        text: String,
    },
    #[command(name = "start")]
    LinkStart {
        read_pattern: String,
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    #[command(name = "stop")]
    LinkStop {
        #[arg(default_value_t = 0)]
        number: usize,
    },
    #[command(name = "view")]
    LinkView {
        #[arg(default_value_t = 0)]
        number: usize,
        #[arg(short, long)]
        absolute: bool,
    },
    #[command(name = "ledger")]
    LinkLedger,
    #[command(subcommand)]
    Workspace(WorkspaceCommand),
}

/// Server command sub-command options
#[derive(Subcommand)]
enum ServerCommand {
    Run,
    Start,
    Stop,
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    Save {
        name: String,
        #[arg(short, long)]
        description: Option<String>,
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
    Load { name: String },
    List,
    View {
        name: String,
        #[arg(short, long)]
        absolute: bool,
    },
    Current,
    Delete { name: String },
    Rename { orig: String, new: String},
}

/// Main entry for the CLI
pub fn entry() -> Result<String, String> {
    // Ensure all necessary folders are created
    ensure_app_dir();
    ensure_workspace_dir();

    // Get the CLI arguments and remove the first one, which is the "python" command
    let mut cli_args: Vec<String> = env::args().collect();
    cli_args.remove(0);

    // Parse the corrected CLI arguments and perform the appropriate action
    let cli = Cli::parse_from(cli_args);
    match cli.command {
        Command::Server(server_command) => server_subentry(server_command),
        Command::Workspace(workspace_command) => workspace_subentry(workspace_command),
        Command::Ping => crate::tcp::client::ping(),
        Command::Echo { text } => crate::tcp::client::echo(text),
        Command::LinkStart {
            read_pattern,
            mut path,
        } => {
            // If no path is provided, attempt to find the connected CircuitPython board
            if path.is_none() {
                path = find_circuitpy();
            }

            // If the path is still not found, return as an error
            if path.is_none() {
                return Err(String::from(
                    "Could not locate a connected CircuitPython board",
                ));
            }

            // Start the link with the provided information via request to server
            crate::tcp::client::start_monitor(
                read_pattern,
                absolute(path.unwrap()).expect("Could not get the current directory"),
                env::current_dir().expect("Could not get the current directory"),
            )
        }
        Command::LinkStop { number } => crate::tcp::client::stop_monitor(number),
        Command::LinkView { number, absolute } => {
            crate::tcp::client::view_monitor(number, absolute)
        }
        Command::LinkLedger => Err(String::from("WIP")),
    }
}

/// Server command subentry, for performing the appropriate command
fn server_subentry(server_command: ServerCommand) -> Result<String, String> {
    match server_command {
        ServerCommand::Run => Ok(crate::tcp::server::run_server()),
        ServerCommand::Start => Ok(crate::tcp::server::start_server()),
        ServerCommand::Stop => crate::tcp::client::stop_server(),
    }
}

/// Workspace command subentry, for performing the appropriate command
fn workspace_subentry(workspace_command: WorkspaceCommand) -> Result<String, String> {
    match workspace_command {
        WorkspaceCommand::Save { name, description, force } => {
            let desc =  description.unwrap_or_default();
            crate::tcp::client::save_workspace(&name, &desc, force)
        },
        WorkspaceCommand::Load { name } => crate::tcp::client::load_workspace(&name),
        WorkspaceCommand::List => crate::workspace::list_workspaces(),
        WorkspaceCommand::View { name, absolute } => crate::workspace::view_workspace(&name, absolute),
        WorkspaceCommand::Current => crate::tcp::client::get_current_workspace(),
        WorkspaceCommand::Delete { name } => crate::workspace::delete_workspace(&name),
        WorkspaceCommand::Rename { orig, new } => crate::workspace::rename_workspace(&orig, &new),
    }
}

pub fn get_app_dir() -> PathBuf {
    let config_dir = dirs::config_dir().expect("Could not locate config directory");
    config_dir.join(env!("CARGO_PKG_NAME"))
}

pub fn ensure_app_dir() {
    let dir = get_app_dir();
    fs::create_dir_all(dir).expect("Could not create application directory");
}

#[cfg(all(feature = "test-support", test))]
mod test {

    #[test]
    fn get_app_dir() {
        let app_dir = crate::cli::get_app_dir();
        assert!(app_dir.ends_with(env!("CARGO_PKG_NAME")))
    }

    mod ensure_app_dir {

        #[test]
        #[serial_test::serial]
        fn successes() {
            let preexisted = crate::test_support::prepare_fresh_state();
            let app_dir = crate::cli::get_app_dir();
            assert!(app_dir.exists());
            crate::test_support::restore_previous_state(preexisted);
        }
    }
}
