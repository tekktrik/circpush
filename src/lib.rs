// SPDX-FileCopyrightText: 2025 Alec Delaney
// SPDX-License-Identifier: MIT

mod board;
mod commands;
mod filetree;
mod link;
mod monitor;
mod tcp;
mod workspace;

use std::path::PathBuf;
use std::{env, path::absolute};

use filetree::{ensure_port_dir, ensure_workspace_dir};
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

/// Push files to a connected CircuitPython board as they are updated locally
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Main CLI command options
#[derive(Subcommand)]
enum Command {
    /// Server-specific commands (e.g., start and stop)
    #[command(subcommand)]
    Server(ServerCommand),
    /// Ping the server
    Ping {
        /// The TCP port to use for pinging the server
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Start a file monitor for a given filename or glob pattern
    #[command(name = "start")]
    LinkStart {
        /// The filename or glob pattern to monitor
        read_pattern: String,
        /// Use a given path as the write location instead of the connected CircuitPython board
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Stop a file monitor
    #[command(name = "stop")]
    LinkStop {
        /// The file monitor number
        #[arg(default_value_t = 0)]
        number: usize,
    },
    /// View the details of a file monitor
    #[command(name = "view")]
    LinkView {
        /// The file monitor number
        #[arg(default_value_t = 0)]
        number: usize,
        /// Display the filepaths as absolute
        #[arg(short, long)]
        absolute: bool,
    },
    /// View all currently monitored files
    #[command(name = "ledger")]
    LinkLedger,
    /// Workspace-specific commands (e.g., save and load)
    #[command(subcommand)]
    Workspace(WorkspaceCommand),
}

/// Server command sub-command options
#[derive(Subcommand)]
enum ServerCommand {
    /// Run the server in the current process
    Run {
        /// The TCP port to use for the server
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Start the server in a new process
    Start {
        /// The TCP port to use for the server
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Stop the server
    Stop,
}

/// FDSNJKFDSNJ
#[derive(Subcommand)]
enum WorkspaceCommand {
    /// Save the current set of file monitors as a workspace
    Save {
        /// The name of the workspace
        name: String,
        #[arg(short, long)]
        /// A description of the workspace
        description: Option<String>,
        /// Overwrite any existing workspace of the same name
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
    /// Load a saved workspace
    Load {
        /// The name of the workspace
        name: String,
    },
    /// List all saved workspaces
    List,
    /// View information about a given workspace
    View {
        /// The name of the workspace
        name: String,
        /// Display the filepaths as absolute
        #[arg(short, long)]
        absolute: bool,
    },
    /// See information about the current workspace
    Current,
    /// Delete a saved workspace
    Delete {
        // The name of the workspace
        name: String,
    },
    /// Rename a saved workspace
    Rename {
        /// The current name of the workspace
        orig: String,
        /// The new name of the workspace
        new: String,
    },
}

/// Main entry for the CLI
pub fn entry(cli_args: &[String]) -> Result<String, String> {
    // Ensure all necessary folders are created
    ensure_app_dir();
    ensure_port_dir();
    ensure_workspace_dir();

    // Parse the corrected CLI arguments and perform the appropriate action
    let cli = Cli::parse_from(cli_args);
    match cli.command {
        Command::Server(server_command) => server_subentry(server_command),
        Command::Workspace(workspace_command) => workspace_subentry(workspace_command),
        Command::Ping { port } => crate::tcp::client::ping(port),
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
        ServerCommand::Run { port } => {
            if crate::tcp::server::is_server_running() {
                return Err(String::from("Server already running"));
            }
            let port = port.unwrap_or_default();
            Ok(crate::tcp::server::run_server(port)?)
        }
        ServerCommand::Start { port } => {
            if crate::tcp::server::is_server_running() {
                return Err(String::from("Server already running"));
            }
            let port = port.unwrap_or_default();
            crate::tcp::server::start_server(port)
        }
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

/// Functionality provided for helping with testing
#[cfg(feature = "test-support")]
pub mod test_support {

    use super::*;

    use std::thread;

    use crate::tcp::server;

    use std::{
        fs,
        path::{Path, PathBuf},
    };

    /// The test configuration directory name
    pub const TEST_APP_DIRECTORY_NAME: &str = ".circpush-test";

    /// Test helper function for starting the server
    pub fn start_server() {
        thread::spawn(|| {
            let _resp = server::run_server(0);
        });
        while tcp::client::ping(None).is_err() {}
    }

    /// Test helper function for stopping the server
    pub fn stop_server() {
        tcp::client::stop_server().expect("Could not stop server");
        while tcp::client::ping(None).is_ok() {}
    }

    /// Test helper function for getting the test configuration directory filepath
    fn get_test_directory() -> PathBuf {
        crate::filetree::get_app_dir().with_file_name(TEST_APP_DIRECTORY_NAME)
    }

    /// Test helper function for ensuring the existing application directory is saved before
    /// running tests that would interfere with its existing state.
    ///
    /// Returns whether the application directory existed.
    ///
    /// Used in combination with restore_app_directory()
    pub fn save_app_directory() -> bool {
        // Get the applciation and test configuration directories
        let app_directory = crate::filetree::get_app_dir();
        let test_directory = get_test_directory();

        // Check whether the application directory exists
        let preexists = app_directory.as_path().is_dir();

        // If the applciation directory exists, move it into the test application configuration directory
        if preexists {
            let copy_options = fs_extra::dir::CopyOptions::new();
            fs::create_dir(&test_directory).expect("Could not create test application directory");
            fs_extra::dir::move_dir(&app_directory, &test_directory, &copy_options)
                .expect("Could not rename existing application directory");
        }

        // Ensure the application and workspace directories are recreated
        crate::filetree::ensure_app_dir();
        crate::filetree::ensure_port_dir();
        crate::filetree::ensure_workspace_dir();

        // Returns whether the application directory existed before creating the fresh install
        preexists
    }

    /// Test helper function for restoring the prior application directory after running tests
    /// that would have interfered with its prior state.
    ///
    /// Used in combination with save_app_directory()
    pub fn restore_app_directory() {
        // Get the applciation and test configuration directories
        let app_directory = crate::filetree::get_app_dir();
        let test_directory = get_test_directory();

        // Remove the current application directory
        fs::remove_dir_all(crate::filetree::get_app_dir())
            .expect("Could not delete test directory");

        // Move the prior application directory (in the test configuration directory folder)
        // back to its prior location as the application directory
        let copy_options = fs_extra::dir::CopyOptions::new();
        fs_extra::dir::move_dir(
            &test_directory.join(env!("CARGO_PKG_NAME")),
            &app_directory.parent().expect("Could not get config folder"),
            &copy_options,
        )
        .expect("Could not restore application directory");

        // Remove that test configuration directory
        fs::remove_dir_all(test_directory).expect("Could not delete test application folder");
    }

    /// Test helper function for ensuring the existing application directory is saved before
    /// running tests that would interfere with its existing state.  It also starts the server
    /// in another process, essentially creating a "fresh install" state.
    ///
    /// Returns whether the application directory existed.
    ///
    /// Used in combination with restore_previous_state()
    pub fn prepare_fresh_state() -> bool {
        let preexists = save_app_directory();
        start_server();
        while tcp::client::ping(None).is_err() {}
        preexists
    }

    /// Test helper function for restoring the prior application directory after running tests
    /// that would have interfered with its prior state, depending on whether it needs to be
    /// restored.  It also stops the server running in another process.
    ///
    /// Used in combination with prepare_fresh_state()
    pub fn restore_previous_state(preexisted: bool) {
        stop_server();
        if preexisted {
            restore_app_directory();
        }
    }

    /// Test helper function for parsing table components out of a response message
    pub fn parse_contents(response: &str, has_nontable_line: bool) -> Vec<Vec<String>> {
        // Create a new list for storing table parts for each row
        let mut all_parts = Vec::new();

        // Iterate through the lines of the table
        for (index, line) in response.trim().lines().enumerate() {
            // Ignore rows that at the table lines
            if line.starts_with("+") {
                continue;
            }

            // Create a new list for storing the column values for this row
            let mut line_parts = Vec::new();

            // If the table has a non-table line and it is the first row, immediately add its
            // contents and move on to the next row of the response
            if index == 0 && has_nontable_line {
                line_parts.push(line.to_owned());
                all_parts.push(line_parts);
                continue;
            }

            // Add the values for each column to the row list
            for part in line.split("|") {
                line_parts.push(part.trim().to_string())
            }

            // Ignore the outsides of the table
            let (_first, line_parts) = line_parts
                .split_first()
                .expect("Could not remove the first element");
            let (_last, line_parts) = line_parts
                .split_last()
                .expect("Could not remove the last element");

            // Add the row of components to the list of all row components
            all_parts.push(line_parts.to_vec());
        }

        // Return the list of all table parts for each row
        all_parts
    }

    /// Test helper function for generating the expected list of table components for all rows of said table
    pub fn generate_expected_parts<B, W>(
        path_components: &[(B, W)],
        monitor_num: usize,
        nontable_line: Option<&str>,
    ) -> Vec<Vec<String>>
    where
        B: AsRef<Path>,
        W: AsRef<Path>,
    {
        // Create a new list for storing table components (per row)
        let mut components = Vec::new();

        // If a non-table line should be added, add it at the top
        if let Some(name_line_str) = nontable_line {
            components.push(vec![name_line_str.to_owned()]);
        }

        // Store the expected header list and add it to the list of the whole table
        let header_str = vec![
            "Link #",
            "Read Pattern",
            "Base Directory",
            "Write Directory",
        ];
        let header = header_str.iter().map(|e| e.to_string()).collect();
        components.push(header);

        // Iterate through the pairs of base and write directories provided
        for (index, (base_directory, write_directory)) in path_components.iter().enumerate() {
            // If a specific file monitor was requiested, the loop only iterates once and that value should bec
            // present in the table
            let number_str = if monitor_num == 0 {
                let number = index + 1;
                number.to_string()
            } else {
                monitor_num.to_string()
            };

            // Create a list of the components for the row, seeding the monitor number and read pattern
            let mut components_str = vec![&number_str, "test*"];

            // Add the base directory to the components for the row
            components_str.push(
                base_directory
                    .as_ref()
                    .to_str()
                    .expect("Could not convert path to string"),
            );

            // Add the write directory to the components for the row
            components_str.push(
                write_directory
                    .as_ref()
                    .to_str()
                    .expect("Could not convert path to string"),
            );

            // Add the components for the row to the list of components for the table
            let line_components = components_str.iter().map(|e| e.to_string()).collect();
            components.push(line_components);
        }

        // Return the list of components for the table
        components
    }
}
