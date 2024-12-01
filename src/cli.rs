use std::{env, path::absolute};
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::board::find_circuitpy;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum ServerCommand {
    Run,
    Start,
    Stop,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    Server(ServerCommand),
    Ping,
    Echo { text: String},
    #[command(name = "start")]
    LinkStart {
        read_pattern: String,
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    #[command(name = "stop")]
    LinkStop {
        #[arg(default_value_t = 0)]
        number: usize
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

pub fn entry() -> Result<String, String> {
    let mut cli_args: Vec<String> = env::args().collect();
    cli_args.remove(0);
    let cli = Cli::parse_from(cli_args);
    match cli.command {
        Command::Server(server_command) => server_subentry(server_command),
        Command::Ping => crate::tcp::client::ping(),
        Command::Echo { text } => crate::tcp::client::echo(text),
        Command::LinkStart { read_pattern, mut path } => {
            if path.is_none() {
                path = find_circuitpy();
            }
            if path.is_none() {
                return Err(String::from("Could not locate a connected CircuitPython board"));
            }
            crate::tcp::client::start_link(
                read_pattern,
                absolute(path.unwrap()).expect("Could not get the current directory"),
                env::current_dir().expect("Could not get the current directory"),
            )
        },
        Command::LinkStop { number } => crate::tcp::client::stop_link(number),
        Command::LinkView { number , absolute} => crate::tcp::client::view_link(number, absolute),
        Command::LinkLedger => Err(String::from("WIP")),

    }
}

fn server_subentry(server_command: ServerCommand) -> Result<String, String> {
    match server_command {
        ServerCommand::Run => Ok(crate::tcp::server::run_server()),
        ServerCommand::Start => Ok(crate::tcp::server::start_server()),
        ServerCommand::Stop => crate::tcp::client::stop_server(),
    }
}