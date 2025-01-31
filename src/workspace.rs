use std::path::PathBuf;
use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::filetree::get_workspace_dir;
use crate::monitor::{as_table, FileMonitor};

/// A workspace consisting of a list of file monitors and a description
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub desc: String,
    pub monitors: Vec<FileMonitor>,
}

/// The ways in which a workspace can fail to load
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceLoadError {
    /// The format of the workspace file is incorrect
    UnexpectedFormat,
    /// The desired workspace file does not exist
    DoesNotExist,
}

/// The ways in which a workspace can fail to save
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceSaveError {
    /// The workspace fails to save to a file
    BadFileSave,
    /// The desired workspace already exists
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
        if !filepath.is_file() {
            return Err(WorkspaceLoadError::DoesNotExist);
        }
        let contents = fs::read_to_string(filepath).expect("Could not read file contents");
        match serde_json::from_str(&contents) {
            Ok(x) => Ok(x),
            Err(_) => Err(WorkspaceLoadError::UnexpectedFormat),
        }
    }

    /// Load a Workspace saved as a given name in the workspace folder
    pub fn from_name(name: &str) -> Result<Self, WorkspaceLoadError> {
        let filepath = get_workspace_dir().join(PathBuf::from(name).with_extension("json"));
        Workspace::from_filepath(&filepath)
    }

    /// Save a Workspace as a file at the given filepath
    pub fn save_as_filepath<P>(&self, filepath: P) -> Result<(), WorkspaceSaveError>
    where
        P: AsRef<Path>,
    {
        // Create the new workspace file
        let writer = match fs::File::create(filepath.as_ref()) {
            Ok(writer) => writer,
            Err(_) => return Err(WorkspaceSaveError::BadFileSave),
        };

        // Get the file monitors without file links
        let mut linkless = self.clone();
        linkless
            .monitors
            .iter_mut()
            .for_each(|m| *m = m.clone_linkless());

        // Pretty print save the Workspace JSON object
        serde_json::to_writer_pretty(writer, &linkless)
            .expect("Could not delete file after failing to create workspace file");

        Ok(())
    }

    /// Save a Workspace in the workspace folder with the given name
    pub fn save_as_name(&self, name: &str, overwrite: bool) -> Result<(), WorkspaceSaveError> {
        // Get the filepath for a Workspace with this name
        let filepath = Workspace::get_filepath_for_name(name);

        // Check whether the workspace file already exists
        let preexists = filepath.is_file();

        // If the workspace file already exists and --force is not true, return an error
        if preexists && !overwrite {
            return Err(WorkspaceSaveError::AlreadyExists);
        }

        // Save the Workspace
        self.save_as_filepath(&filepath)
    }

    /// Get the filename for a Workspace with the given name
    pub fn get_filepath_for_name(name: &str) -> PathBuf {
        let mut filepath = get_workspace_dir().join(name);
        filepath.set_extension("json");
        filepath
    }
}

/// Command handler for listing all workspaces
pub fn list_workspaces() -> Result<String, String> {
    // Create a new list for appending workspace names
    let mut workspace_names = Vec::new();

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
        workspace_names.push(
            workspace_name
                .to_str()
                .expect("Could not convert filestem to string")
                .to_owned(),
        );
    }

    // If no workspaces were found, return this to the user
    if workspace_names.is_empty() {
        Ok(String::from("No workspaces have been saved"))
    }
    // Otherwise remove the last newline and return the built string
    else {
        // Sort the workspace names alphabetically
        workspace_names.sort();

        // Create the string from the sorted workspace names
        let mut workspace_msg = String::new();
        workspace_names.iter().for_each(|n| {
            workspace_msg.push_str(n);
            workspace_msg.push('\n');
        });

        // Remove the last new line and return the built string
        workspace_msg.pop();
        Ok(workspace_msg)
    }
}

/// Rename a workspace file
pub fn rename_workspace(orig: &str, new: &str) -> Result<String, String> {
    // Get the filepaths for the current and new workspace file
    let orig_filepath = Workspace::get_filepath_for_name(orig);
    let new_filepath = Workspace::get_filepath_for_name(new);

    // Return an error if the requested origin workspace file does not exist
    if !orig_filepath.is_file() {
        return Err(format!("Workspace '{orig}' does not exist"));
    }

    // Rename the workspace file
    fs::rename(&orig_filepath, &new_filepath).expect("Could not rename the workspace");
    Ok(format!("Renamed workspace '{orig}' to '{new}'"))
}

/// Delete a workspace file
pub fn delete_workspace(name: &str) -> Result<String, String> {
    // Get the filepath for the given workspace name
    let filepath = Workspace::get_filepath_for_name(name);

    // Return an error if the requested workspace file does not exist
    if !filepath.is_file() {
        return Err(format!("Workspace '{name}' does not exist"));
    }

    // Delete the workspace file
    fs::remove_file(&filepath).expect("Could not delete the workspace");
    Ok(format!("Deleted workspace '{name}'"))
}

/// View a workspace file with the given name
pub fn view_workspace(name: &str, absolute: bool) -> Result<String, String> {
    // Get the Workspace with the given name
    let workspace = match Workspace::from_name(name) {
        Ok(workspace) => workspace,
        Err(WorkspaceLoadError::UnexpectedFormat) => {
            return Err(format!("Could not parse the format of workspace '{name}'"))
        }
        Err(WorkspaceLoadError::DoesNotExist) => {
            return Err(format!("Workspace '{name}' does not exist"))
        }
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

#[cfg(all(test, feature = "test-support"))]
mod test {

    use tempfile::TempDir;

    use super::*;

    /// Helper function for generating a file monitor
    fn get_monitor() -> FileMonitor {
        let write_directory = TempDir::new().expect("Could not create temporary write directory");
        let base_directory = TempDir::new().expect("Could not create temporary base directory");
        FileMonitor::new("test*", write_directory.path(), base_directory.path())
    }

    /// Helper function for creating a workspace
    fn get_workspace() -> Workspace {
        let monitor = get_monitor();
        let monitors = vec![monitor];
        Workspace {
            desc: String::from("Example"),
            monitors,
        }
    }

    /// Tests creating a new workspace
    #[test]
    fn new() {
        // Generate a cookiecutter workspace
        let template_workspace = get_workspace();

        // Store the information from the cookiecutter workspace
        let desc = &template_workspace.desc;
        let monitors = &template_workspace.monitors;

        // Create the workspace using Workspace::new()
        let workspace: Workspace = Workspace::new(desc, monitors);

        // Check the equality of the workspaces and their fields
        assert_eq!(workspace, template_workspace);
        assert_eq!(workspace.desc, template_workspace.desc);
        assert_eq!(workspace.monitors, template_workspace.monitors);
    }

    mod from_filepath {

        use std::io::Write;

        use super::*;

        /// Tests the successful generation of a workspace from a filepath
        #[test]
        fn success() {
            // Load the test asset workspace
            let filepath = PathBuf::from("tests/assets/workspaces/testws.json");
            let workspace: Workspace =
                Workspace::from_filepath(&filepath).expect("Could not get workspace from filepath");

            // Generate the expected contained file monitor
            let write_directory = PathBuf::from("/circpush/tests/assets/sandbox/");
            let base_directory = PathBuf::from("/circpush");
            let monitor = FileMonitor::new("test*", &write_directory, &base_directory);

            // Generate the expected workspace fields
            let expected_desc = "A test workspace";
            let expected_monitors = vec![monitor];

            // Check that the workspace fields matched the expected values
            assert_eq!(&workspace.desc, expected_desc);
            assert_eq!(workspace.monitors, expected_monitors);
        }

        /// Tests attempting to get a workspace when the workspace file does not exist
        #[test]
        fn bad_file_read_error() {
            let filepath = PathBuf::from("/does/not/exist");
            let error = Workspace::from_filepath(&filepath)
                .expect_err("Successfully loaded workspace from filepath");
            assert_eq!(error, WorkspaceLoadError::DoesNotExist);
        }

        /// Tests attempting to get a workspace from an incorrectly formatted file
        #[test]
        fn unexpected_format_error() {
            // Create a temporary file to be used as the workspace file
            let mut temp_file =
                tempfile::NamedTempFile::new().expect("Could not get temporary file");

            // Write data not properly formatted for a workspace file
            temp_file
                .write(b"junkdata")
                .expect("Could not write to temporary file");

            // Check that attempt to get a workspace from the file returns an error
            let error = Workspace::from_filepath(temp_file.path())
                .expect_err("Successfully loaded workspace from filepath");
            assert_eq!(error, WorkspaceLoadError::UnexpectedFormat);
        }
    }

    mod from_name {

        use super::*;

        /// Tests the successful generation of a workspace from a name
        #[test]
        #[serial_test::serial]
        fn success() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the name of the worskpace
            let filename = "testws";

            // Get the expected workspace filepath for the given name
            let mut workspace_filepath = get_workspace_dir().join(filename);
            workspace_filepath.set_extension("json");

            // Store the filepath of the test asset workspace file
            let test_filepath = PathBuf::from("tests/assets/workspaces/testws.json");

            // Copy the test asset file contents to the expected workspace file
            fs::File::create_new(&workspace_filepath)
                .expect("Could not create new mock workspace file");
            fs::copy(&test_filepath, &workspace_filepath)
                .expect("Could not copy the mock workspace file");

            // Check that the workspace can be retrieved by name
            let _workspace: Workspace =
                Workspace::from_name(&filename).expect("Could not retrieve the workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);
        }

        /// Tests that the retrieving a nonexistent workspace causes an error
        #[test]
        #[serial_test::serial]
        fn error() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the name of the worskpace
            let filename = "doesnotexist";

            // Check that the workspace cannot be reireve by name
            let error =
                Workspace::from_name(&filename).expect_err("Successfully retrieved the workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the error returned is the correct one
            assert_eq!(error, WorkspaceLoadError::DoesNotExist);
        }
    }

    mod save_as_filepath {

        use std::iter::zip;

        use super::*;

        /// Tests the successful saving of a workspace to a filepath
        #[test]
        fn success() {
            // Get the filepath of the test asset workspace file to load
            let load_filepath = PathBuf::from("tests/assets/workspaces/testws.json");

            // Load the test asset workspace
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            // Create a temporary directory
            let tempdir = TempDir::new().expect("Could not get temporary directory");

            // Get the save filepath for the workspace
            let save_filepath = tempdir.path().join("testsave");

            // Save the workspace to the save filepath
            workspace
                .save_as_filepath(&save_filepath)
                .expect("Could not save the workspace");

            // Load the file contents of the loaded and saved workspace files
            let loaded = fs::read_to_string(&load_filepath)
                .expect("Could not load contents of test workspace");
            let saved = fs::read_to_string(&save_filepath)
                .expect("Could not load contents of saved workspace");

            // Check that the file contents of each workspace file match
            for (saved_line, loaded_line) in zip(saved.trim().lines(), loaded.trim().lines()) {
                assert_eq!(saved_line, loaded_line);
            }
        }

        /// Tests that a bad file save of the workspace causes an error
        #[test]
        fn bad_file_save() {
            // Create a new workspace
            let workspace: Workspace = Workspace::new("desc", &[]);

            // Store a filepath that does not exist
            let filepath = PathBuf::from("/does/not/exist");
            assert!(!filepath.as_path().exists());

            // Check that saving the workspace results in an error
            let error = workspace
                .save_as_filepath(&filepath)
                .expect_err("Successfully saved the workspace");
            assert_eq!(error, WorkspaceSaveError::BadFileSave);
        }
    }

    mod save_as_name {

        use std::iter::zip;

        use super::*;

        /// Tests the successful saving of a workspace using a name when it is a new workspace
        #[test]
        #[serial_test::serial]
        fn new() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the workspace name
            let filename = "testws";

            // Get the filepath of the test asset workspace file to load
            let load_filepath = PathBuf::from(format!("tests/assets/workspaces/{filename}.json"));
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            // Get the intended save filepath for the test asset workspace file
            let mut save_filepath = get_workspace_dir().join(&filename);
            save_filepath.set_extension("json");

            // Save the workspace with the given name
            workspace
                .save_as_name(&filename, false)
                .expect("Could not save workspace");

            // Load the file contents of the loaded and saved workspace files
            let loaded = fs::read_to_string(&load_filepath)
                .expect("Could not load contents of test workspace");
            let saved = fs::read_to_string(&save_filepath)
                .expect("Could not load contents of saved workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the file contents of each workspace file match
            for (saved_line, loaded_line) in zip(saved.trim().lines(), loaded.trim().lines()) {
                assert_eq!(saved_line, loaded_line);
            }
        }

        /// Tests the successful saving of a workspace using a name when it is overwriting an existing workspace
        #[test]
        #[serial_test::serial]
        fn existing() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the workspace name
            let filename = "testws";

            // Get the filepath of the test asset workspace file to load
            let load_filepath = PathBuf::from(format!("tests/assets/workspaces/{filename}.json"));
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            // Get the intended save filepath for the test asset workspace file
            let mut save_filepath = get_workspace_dir().join(&filename);
            save_filepath.set_extension("json");

            // Create a copy of the workspace file at the intended save filepath
            fs::File::create_new(&save_filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &save_filepath).expect("Could not copy file contents");

            // Save the workspace with the given name, overwriting the old workspace file
            workspace
                .save_as_name(&filename, true)
                .expect("Could not save workspace");

            // Load the file contents of the loaded and saved workspace files
            let loaded = fs::read_to_string(&load_filepath)
                .expect("Could not load contents of test workspace");
            let saved = fs::read_to_string(&save_filepath)
                .expect("Could not load contents of saved workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the file contents of each workspace file match
            for (saved_line, loaded_line) in zip(saved.trim().lines(), loaded.trim().lines()) {
                assert_eq!(saved_line, loaded_line);
            }
        }

        /// Tests the successful saving of a workspace using a name when it already exists
        #[test]
        #[serial_test::serial]
        fn already_exists_error() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the workspace name
            let filename = "testws";

            // Get the filepath of the test asset workspace file to load
            let load_filepath = PathBuf::from(format!("tests/assets/workspaces/{filename}.json"));
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            // Get the intended save filepath for the test asset workspace file
            let mut save_filepath = get_workspace_dir().join(&filename);
            save_filepath.set_extension("json");

            // Create a copy of the workspace file at the intended save filepath
            fs::File::create_new(&save_filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &save_filepath).expect("Could not copy file contents");

            // Attempt to save the workspace with the given name without forcing overwrites
            let error = workspace
                .save_as_name(&filename, false)
                .expect_err("Successfully saved existing workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the correct error was returned
            assert_eq!(error, WorkspaceSaveError::AlreadyExists);
        }
    }

    mod list_workspaces {

        use super::*;

        /// Tests the successful listing of all saved workspaces
        #[test]
        #[serial_test::serial]
        fn list_all() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Create a list for storing listed workspace names
            let mut intended_resps = Vec::new();

            // Create multiple test files in the workspace directory and add it to the list of
            // expected workspace names to be listed
            for i in 0..3 {
                let name = format!("test{i}");
                let filepath = get_workspace_dir().join(format!("{name}.json"));
                fs::File::create_new(&filepath).expect("Could not create new file");
                intended_resps.push(name);
            }

            // Create a directory in the workspace directory, which should be ignored
            let ignored_directory = get_workspace_dir().join("junkfolder");
            fs::create_dir(&ignored_directory).expect("Could not create the directory");

            // List the workspaces
            let response = list_workspaces().expect("Could not get the list of workspaces");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Parse the response into the workspace names
            let returned_resps: Vec<String> = response
                .as_str()
                .split("\n")
                .map(|x| x.trim().to_string())
                .collect();

            // Check the returned workspaec names match the intended ones
            assert_eq!(returned_resps, intended_resps);
        }

        /// Tests the listing of all saved workspaces when none are saved
        #[test]
        #[serial_test::serial]
        fn none_saved() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the expected response message
            let expected = "No workspaces have been saved";

            // List all workspaces
            let response = list_workspaces().expect("Could not get the list of workspaces");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check the returned response message matched the expected one
            assert_eq!(&response, expected);
        }
    }

    mod rename_workspace {

        use super::*;

        /// Tests the successful renaming of a workspace
        #[test]
        #[serial_test::serial]
        fn success() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Create multiple test files in the workspace directory
            for i in 0..3 {
                let filepath = get_workspace_dir().join(format!("test{i}.json"));
                fs::File::create_new(&filepath).expect("Could not create new file");
            }

            // Store the original and new workspace names
            let orig_name = "test1";
            let new_name = "test55";

            // Store the expected response message
            let expected = format!("Renamed workspace '{orig_name}' to '{new_name}'");

            // Get the filepath for the original workspace file
            let mut orig_filepath = get_workspace_dir().join(&orig_name);
            orig_filepath.set_extension("json");

            // Get the filepath for the new workspace file
            let mut new_filepath = get_workspace_dir().join(&new_name);
            new_filepath.set_extension("json");

            // Read the file contents of the original workspace file
            let orig_contents =
                fs::read_to_string(&orig_filepath).expect("Could not read file contents");

            // Rename the workspace
            let response =
                rename_workspace(&orig_name, &new_name).expect("Could not rename workspace");

            // Get the file contents of the new workspace file
            let new_contents =
                fs::read_to_string(&new_filepath).expect("Could not read file contents");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the returned response message matches the expected message
            assert_eq!(response, expected);

            // Check that the file contents have not change after the renaming
            assert_eq!(orig_contents, new_contents);
        }

        /// Tests attempting to rename a workspace when it does not exist
        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the workspace name
            let name = "doesnotexist";

            // Store the expected response message
            let expected = format!("Workspace '{name}' does not exist");

            // Attempt to rename the workspace
            let response = rename_workspace(&name, "newname")
                .expect_err("Successfully renamed nonexistent workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the returned response message matches the expected message
            assert_eq!(response, expected);
        }
    }

    mod delete_workspace {

        use super::*;

        /// Tests the successful deletion of a workspace
        #[test]
        #[serial_test::serial]
        fn success() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Create multiple test files in the workspace directory
            for i in 0..3 {
                let filepath = get_workspace_dir().join(format!("test{i}.json"));
                fs::File::create_new(&filepath).expect("Could not create new file");
            }

            // Store the name and filepath for the workspace to be deleted
            let name = "test1";
            let filepath = Workspace::get_filepath_for_name(&name);

            // Store the expected response message
            let expected = format!("Deleted workspace '{name}'");

            // Delete the workspace
            let response = delete_workspace("test1").expect("Could not delete workspace");

            // Check that the workspace file no longer exists
            assert!(!filepath.as_path().exists());

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the returned response message matches the expected one
            assert_eq!(response, expected);
        }

        /// Tests attempting to delete a workspace when it does not exist
        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            // Store the workspace name
            let name = "doesnotexist";

            let expected = format!("Workspace '{name}' does not exist");
            let response =
                delete_workspace(name).expect_err("Successfully deleted nonexistent workspace");

            assert_eq!(expected, response);
        }
    }

    mod view_workspace {

        use super::*;

        /// Tests the successful viewing of workspace details when it has a description
        #[test]
        #[serial_test::serial]
        fn success_with_description() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the filepath of the test asset workspace with a description
            let load_filepath = PathBuf::from("tests/assets/workspaces/testws.json");

            // Store the name and filepath of the intended workspace
            let name = "withdescription";
            let filepath = Workspace::get_filepath_for_name(&name);

            // Copy the contents of the test asset file to the intended workspace filepath
            fs::File::create_new(&filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &filepath).expect("Could not copy file");

            // Get the expected components from the expected response table
            let base_filepath = PathBuf::from("/circpush");
            let write_filepath = PathBuf::from("/circpush/tests/assets/sandbox/");
            let path_components = [(base_filepath, write_filepath)];
            let expected_components = crate::test_support::generate_expected_parts(
                &path_components,
                0,
                Some(&format!("{name} - A test workspace")),
            );

            // View the workspace
            let response = view_workspace(name, true).expect("Could not view workspace");

            // Parse the components from the returned response table
            let response_components = crate::test_support::parse_contents(&response, true);

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the components in the returned and expected response tables match
            assert_eq!(response_components, expected_components);
        }

        /// Tests the successful viewing of workspace details when it does not have a description
        #[test]
        #[serial_test::serial]
        fn success_without_description() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the filepath of the test asset workspace without a description
            let load_filepath = PathBuf::from("tests/assets/workspaces/nodescription.json");

            // Store the name and filepath of the intended workspace
            let name = "nodescription";
            let filepath = Workspace::get_filepath_for_name(&name);

            // Copy the contents of the test asset file to the intended workspace filepath
            fs::File::create_new(&filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &filepath).expect("Could not copy file");

            // Get the expected components from the expected response table
            let base_filepath = PathBuf::from("/circpush");
            let write_filepath = PathBuf::from("/circpush/tests/assets/sandbox/");
            let path_components = [(base_filepath, write_filepath)];
            let expected_components =
                crate::test_support::generate_expected_parts(&path_components, 0, Some(name));

            // View the workspace
            let response = view_workspace(name, true).expect("Could not view workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Parse the components from the returned response table
            let response_components = crate::test_support::parse_contents(&response, true);

            // Check that the components in the returned and expected response tables match
            assert_eq!(response_components, expected_components);
        }

        /// Tests attempting to view workspace details when it does not exist
        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the name of the nonexistent workspace
            let name = "doesnotexist";

            // Store the expected response message
            let expected = format!("Workspace '{name}' does not exist");

            // Attempt to view the workspace
            let response =
                view_workspace(name, true).expect_err("Successfully viewed nonexistent workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the components in the returned and expected response tables match
            assert_eq!(response, expected);
        }

        /// Tests attempting to view workspace details when it is formatted incorrectly
        #[test]
        #[serial_test::serial]
        fn unexpected_format_error() {
            // Save the existing state of the application directory
            let preexisted = crate::test_support::prepare_fresh_state();

            // Store the name and filpath of the intended workspace with an incorrect format
            let name = "badformat";
            let filepath = Workspace::get_filepath_for_name(&name);

            // Create an empty (incorrectly formatted) file for the workspace
            fs::File::create_new(&filepath).expect("Could not create new file");

            // Store the expected response message
            let expected = format!("Could not parse the format of workspace '{name}'");

            // Attempt to view the workspace
            let response = view_workspace(name, true)
                .expect_err("Successfully viewed incorrectly formatted workspace");

            // Restore the previous state of the application directory
            crate::test_support::restore_previous_state(preexisted);

            // Check that the components in the returned and expected response tables match
            assert_eq!(response, expected);
        }
    }
}
