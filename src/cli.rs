use std::path::PathBuf;
use std::{env, path::absolute};

use clap::{Parser, Subcommand};

use crate::board::find_circuitpy;

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
}

/// Server command sub-command options
#[derive(Subcommand)]
enum ServerCommand {
    Run,
    Start,
    Stop,
}

/// Main entry for the CLI
pub fn entry() -> Result<String, String> {
    // Get the CLI arguments and remove the first one, which is the "python" command
    let mut cli_args: Vec<String> = env::args().collect();
    cli_args.remove(0);

    // Parse the corrected CLI arguments and perform the appropriate action
    let cli = Cli::parse_from(cli_args);
    match cli.command {
        Command::Server(server_command) => server_subentry(server_command),
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
