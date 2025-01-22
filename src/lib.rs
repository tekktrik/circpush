mod board;
mod commands;
mod filetree;
mod link;
mod monitor;
mod tcp;
mod workspace;

use std::path::PathBuf;
use std::{env, path::absolute};

use filetree::ensure_workspace_dir;
use pyo3::prelude::*;
use std::process::exit;

use clap::{Parser, Subcommand};

use crate::board::find_circuitpy;
use crate::filetree::ensure_app_dir;

/// Python module created using PyO3 (circpush)
#[pymodule]
pub mod circpush {

    use std::env;

    use super::*;

    /// Function within the module (cli())
    ///
    /// This is essentially just the PyO3 wrappcer around cli::entry(),
    /// that prints out the resulting text exits with the appropriate
    /// exit code.
    #[pyfunction]
    pub fn cli() -> PyResult<()> {
        // Get the CLI arguments and remove the first one, which is the "python" command
        let mut cli_args: Vec<String> = env::args().collect();
        cli_args.remove(0);

        // Perform the requested action
        match entry(&cli_args) {
            Ok(text) => {
                println!("{text}");
                Ok(())
            }
            Err(text) => {
                println!("{text}");
                exit(1);
            }
        }
    }
}

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
    Load {
        name: String,
    },
    List,
    View {
        name: String,
        #[arg(short, long)]
        absolute: bool,
    },
    Current,
    Delete {
        name: String,
    },
    Rename {
        orig: String,
        new: String,
    },
}

/// Main entry for the CLI
pub fn entry(cli_args: &[String]) -> Result<String, String> {
    // Ensure all necessary folders are created
    ensure_app_dir();
    ensure_workspace_dir();

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
        WorkspaceCommand::Save {
            name,
            description,
            force,
        } => {
            let desc = description.unwrap_or_default();
            crate::tcp::client::save_workspace(&name, &desc, force)
        }
        WorkspaceCommand::Load { name } => crate::tcp::client::load_workspace(&name),
        WorkspaceCommand::List => crate::workspace::list_workspaces(),
        WorkspaceCommand::View { name, absolute } => {
            crate::workspace::view_workspace(&name, absolute)
        }
        WorkspaceCommand::Current => crate::tcp::client::get_current_workspace(),
        WorkspaceCommand::Delete { name } => crate::workspace::delete_workspace(&name),
        WorkspaceCommand::Rename { orig, new } => crate::workspace::rename_workspace(&orig, &new),
    }
}

#[cfg(feature = "test-support")]
pub mod test_support {

    use super::*;

    use std::{
        fs,
        path::{Path, PathBuf},
    };

    pub const TEST_APP_DIRECTORY_NAME: &str = ".circpush-test";

    pub fn stop_server() {
        tcp::client::stop_server().expect("Could not stop server");
    }

    fn get_test_directory() -> PathBuf {
        crate::filetree::get_app_dir().with_file_name(TEST_APP_DIRECTORY_NAME)
    }

    pub fn save_app_directory() -> bool {
        let app_directory = crate::filetree::get_app_dir();
        let test_directory = get_test_directory();

        let preexists = app_directory.exists();
        if preexists {
            let copy_options = fs_extra::dir::CopyOptions::new();

            fs::create_dir(&test_directory).expect("Could not create test application directory");
            fs_extra::dir::move_dir(&app_directory, &test_directory, &copy_options)
                .expect("Could not rename existing application directory");
        }
        crate::filetree::ensure_app_dir();
        crate::filetree::ensure_workspace_dir();
        preexists
    }

    pub fn restore_app_directory() {
        let app_directory = crate::filetree::get_app_dir();
        let test_directory = get_test_directory();

        let copy_options = fs_extra::dir::CopyOptions::new();

        fs::remove_dir_all(crate::filetree::get_app_dir())
            .expect("Could not delete test directory");

        fs_extra::dir::move_dir(
            &test_directory.join(env!("CARGO_PKG_NAME")),
            &app_directory.parent().expect("Could not get config folder"),
            &copy_options,
        )
        .expect("Could not restore application directory");

        fs::remove_dir_all(test_directory).expect("Could not delete test application folder");
    }

    pub fn prepare_fresh_state() -> bool {
        let preexists = save_app_directory();
        tcp::server::start_server();
        while tcp::client::ping().is_err() {}
        preexists
    }

    pub fn restore_previous_state(preexisted: bool) {
        stop_server();
        while tcp::client::ping().is_ok() {}

        if preexisted {
            restore_app_directory();
        }
    }

    pub fn parse_contents(response: &str, has_name_line: bool) -> Vec<Vec<String>> {
        let mut all_parts = Vec::new();
        for (index, line) in response.trim().lines().enumerate() {
            if line.starts_with("+") {
                continue;
            }

            let mut line_parts = Vec::new();

            if index == 0 && has_name_line {
                line_parts.push(line.to_owned());
                all_parts.push(line_parts);
                continue;
            }

            for part in line.split("|") {
                line_parts.push(part.trim().to_string())
            }

            let (_first, line_parts) = line_parts
                .split_first()
                .expect("Could not remove the first element");
            let (_last, line_parts) = line_parts
                .split_last()
                .expect("Could not remove the last element");
            all_parts.push(line_parts.to_vec());
        }

        all_parts
    }

    pub fn generate_expected_parts<B, W>(
        path_components: &[(B, W)],
        link_num: usize,
        name_line: Option<&str>,
    ) -> Vec<Vec<String>>
    where
        B: AsRef<Path>,
        W: AsRef<Path>,
    {
        let mut components = Vec::new();

        if let Some(name_line_str) = name_line {
            components.push(vec![name_line_str.to_owned()]);
        }

        let header_str = vec![
            "Link #",
            "Read Pattern",
            "Base Directory",
            "Write Directory",
        ];
        let header = header_str.iter().map(|e| e.to_string()).collect();

        components.push(header);

        for (index, (base_directory, write_directory)) in path_components.iter().enumerate() {
            let number_str = if link_num == 0 {
                let number = index + 1;
                number.to_string()
            } else {
                link_num.to_string()
            };

            let mut components_str = vec![&number_str, "test*"];
            components_str.push(
                base_directory
                    .as_ref()
                    .to_str()
                    .expect("Could not convert path to string"),
            );
            components_str.push(
                write_directory
                    .as_ref()
                    .to_str()
                    .expect("Could not convert path to string"),
            );

            let line_components = components_str.iter().map(|e| e.to_string()).collect();

            components.push(line_components);
        }

        components
    }
}
