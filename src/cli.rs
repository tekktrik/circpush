use std::env;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

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

// #[derive(Args)]
// struct LinkStartArgs {
//     read_pattern: String,
//     write_directory: PathBuf,
// }

#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    Server(ServerCommand),
    Ping,
    Echo { text: String},
    LinkStart {
        read_pattern: String,
        write_directory: PathBuf,   
    },
    LinkStop {
        #[arg(default_value_t = 0)]
        number: usize
    },
    LinkView {
        #[arg(default_value_t = 0)]
        number: usize
    },
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
        Command::LinkStart { read_pattern, write_directory } => {
            crate::tcp::client::start_link(
                read_pattern,
                write_directory,
                env::current_dir().expect("Could not get the current directory"),
            )
        },
        Command::LinkStop { number } => crate::tcp::client::stop_link(number),
        Command::LinkView { number } => crate::tcp::client::view_link(number),
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