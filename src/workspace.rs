use std::path::PathBuf;
use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::cli::get_app_dir;
use crate::monitor::{as_table, FileMonitor};

pub const WORKSPACE_DIRNAME: &str = "workspaces";

#[derive(Clone, Serialize, Deserialize)]
pub struct Workspace {
    // name: String,
    pub desc: String,
    pub monitors: Vec<FileMonitor>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceLoadError {
    BadFileRead,
    DoesNotExist,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceSaveError {
    BadFileSave,
    AlreadyExists,
}

impl Workspace {
    /// Create a new Workspace with the given information
    pub fn new(desc: &str, monitors: &[FileMonitor]) -> Self {
        Workspace {
            desc: String::from(desc),
            monitors: Vec::from(monitors),
        }
    }

    /// Load a Workspace saved at a given filepath
    pub fn from_filepath(filepath: &Path) -> Result<Self, WorkspaceLoadError> {
        let contents = fs::read_to_string(filepath);
        if contents.is_err() {
            return Err(WorkspaceLoadError::BadFileRead);
        }
        let workspace: Workspace =
            serde_json::from_str(&contents.unwrap()).expect("Failed to parse workspace file");
        Ok(workspace)
    }

    /// Load a Workspace saved as a given name in the workspace folder
    pub fn from_name(name: &str) -> Result<Self, WorkspaceLoadError> {
        let filepath = get_workspace_dir().join(PathBuf::from(name).with_extension("json"));
        if !filepath.is_file() {
            return Err(WorkspaceLoadError::DoesNotExist);
        }
        Workspace::from_filepath(&filepath)
    }

    /// Save a Workspace as a file at the given filepath
    pub fn save_as_filepath(&self, filepath: &Path) -> Result<(), WorkspaceSaveError> {
        // Check whether the workspace file already exists
        let preexists = fs::exists(filepath).expect("Failed to check whether this filepath pre-exists");

        // Create the new workspace file
        let writer = match fs::File::create(filepath) {
            Ok(writer) => writer,
            Err(_) => return Err(WorkspaceSaveError::BadFileSave),
        };

        // Pretty print save the Workspace JSON object
        match serde_json::to_writer_pretty(writer, self) {
            Ok(_) => Ok(()),
            Err(_) => {
                // Delete the recently created file as part of error handling clean-up
                if !preexists {
                    fs::remove_file(filepath).expect("Could not delete file after failing to create workspace file");
                }
                Err(WorkspaceSaveError::BadFileSave)
            },
        }
    }

    /// Save a Workspace in the workspace folder with the given name
    pub fn save_as_name(&self, name: &str, overwrite: bool) -> Result<(), WorkspaceSaveError> {
        // Get the filepath for a Workspace with this name
        let filepath = Workspace::get_filepath_for_name(name);

        // Check whether the workspace file already exists
        let preexists = fs::exists(&filepath).expect("Failed to check whether this filepath pre-exists");

        // If the workspace file already exists and --force is not true, return an error
        if preexists && !overwrite {
            return Err(WorkspaceSaveError::AlreadyExists);
        }

        // Save the Workspace
        self.save_as_filepath(&filepath)
    }

    /// Get the filename for a Workspace with the given name
    fn get_filepath_for_name(name: &str) -> PathBuf {
        let mut filepath = crate::workspace::get_workspace_dir();
        filepath.set_file_name(name);
        filepath.set_extension(".json");
        filepath
    }
}

/// Get the workspace directory path
pub fn get_workspace_dir() -> PathBuf {
    get_app_dir().join(WORKSPACE_DIRNAME)
}

/// Ensure the workspace directory exists
pub fn ensure_workspace_dir() -> Result<(), ()> {
    let dir = get_workspace_dir();
    if fs::create_dir_all(dir).is_err() {
        return Err(());
    }
    Ok(())
}

/// Command handler for listing all workspaces
pub fn list_workspaces() -> Result<String, String> {
    // Create a new string for appending workspace names
    let mut workspace_names = String::new();

    // Iterate through all of the workspace sub-entries
    for entry in get_workspace_dir()
        .read_dir()
        .expect("Could not read the workspaces directory")
        .flatten()
    {
        // Ignore anything that is not a file
        if !entry.path().is_file() {
            continue;
        }

        // Get the name of the workspace and add it to the string
        let entry_path = entry.path();
        let workspace_name = entry_path
            .file_stem()
            .expect("Could not get file stem of workspace file");
        workspace_names.push_str(
            workspace_name
                .to_str()
                .expect("Could not convert filestem to string"),
        );
        workspace_names.push('\n');
    }

    // If no workspaces were found, return this to the user
    if workspace_names.is_empty() {
        Ok(String::from("No workspaces have been saved"))
    }
    // Otherwise remove the last newline and return the built string
    else {
        workspace_names.pop();
        Ok(workspace_names)
    }
}

/// Rename a workspace file
pub fn rename_workspace(orig: &str, new: &str) -> Result<String, String> {
    // Get the filepaths for the current and new workspace file
    let orig_filepath = Workspace::get_filepath_for_name(orig);
    let new_filepath = Workspace::get_filepath_for_name(new);

    // Return an error if the requested origin workspace file does not exist
    if !fs::exists(&orig_filepath).expect("Could not confirm the existence of the workspace file") {
        return Err(format!("Workspace '{orig}' does not exist"));
    }

    // Rename the workspace file
    match fs::rename(&orig_filepath, &new_filepath) {
        Ok(_) => Ok(format!("Renamed workspace '{orig}' to '{new}'")),
        Err(_) => Err(String::from("Could not rename the workspace")),
    }

}

/// Delete a workspace file
pub fn delete_workspace(name: &str) -> Result<String, String> {
    // Get the filepath for the given workspace name
    let filepath = Workspace::get_filepath_for_name(name);

    // Return an error if the requested workspace file does not exist
    if !fs::exists(&filepath).expect("Could not check whether the workspace exists") {
        return Err(format!("Workspace '{name}' does not exist"));
    }

    // Delete the workspace file
    match fs::remove_file(&filepath) {
        Ok(_) => Ok(format!("Deleted workspace '{name}'")),
        Err(_) => Err(String::from("Could not delete the workspace"))
    }
}

/// View a workspace file with the given name
pub fn view_workspace(name: &str, absolute: bool) -> Result<String, String> {
    // Get the Workspace with the given name
    let workspace = match Workspace::from_name(name) {
        Ok(workspace) => workspace,
        Err(WorkspaceLoadError::DoesNotExist) => return Err(format!("Workspace '{name}' does not exist")),
        Err(WorkspaceLoadError::BadFileRead) => return Err(String::from("Could not load the workspace file requested")),
    };

    // Create a new text, seeding it with the name of the workspace
    let mut text = String::from(name);

    // Add the workspace description to the 
    if !workspace.desc.is_empty() {
        let desc = workspace.desc;
        text.push_str(&format!(" - {desc}"));
    }
    text.push('\n');

    // Create the table for the workspace's file monitors and add it to the string
    let table = as_table(&workspace.monitors, 0, absolute);
    text.push_str(&table.to_string());

    // Return the built string
    Ok(text)
}
