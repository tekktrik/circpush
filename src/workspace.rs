use std::path::PathBuf;
use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::filetree::get_workspace_dir;
use crate::monitor::{as_table, FileMonitor};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        Err(WorkspaceLoadError::DoesNotExist) => return Err(format!("Workspace '{name}' does not exist")),
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

    fn get_monitor() -> FileMonitor {
        let write_directory = TempDir::new().expect("Could not create temporary write directory");
        let base_directory = TempDir::new().expect("Could not create temporary base directory");
        FileMonitor::new("test*", write_directory.path(), base_directory.path())
    }

    fn get_workspace() -> Workspace {
        let monitor = get_monitor();
        let monitors = vec![monitor];
        Workspace {
            desc: String::from("Example"),
            monitors,
        }
    }

    #[test]
    fn new() {
        let template = get_workspace();
        let desc = &template.desc;
        let monitors = &template.monitors;
        let monitor: Workspace = Workspace::new(desc, monitors);
        assert_eq!(monitor, template);
        assert_eq!(monitor.desc, template.desc);
        assert_eq!(monitor.monitors, template.monitors);
    }

    mod from_filepath {

        use std::io::Write;

        use super::*;

        #[test]
        fn success() {
            let filepath = PathBuf::from("tests/assets/workspaces/testws.json");
            let workspace: Workspace =
                Workspace::from_filepath(&filepath).expect("Could not get workspace from filepath");

            let write_directory = PathBuf::from("/circpush/tests/assets/sandbox/");
            let base_directory = PathBuf::from("/circpush");
            let monitor = FileMonitor::new("test*", &write_directory, &base_directory);
            let monitors = vec![monitor];

            assert_eq!(&workspace.desc, "A test workspace");
            assert_eq!(&workspace.monitors, &monitors);
        }

        #[test]
        fn bad_file_read_error() {
            let filepath = PathBuf::from("/does/not/exist");
            let error = Workspace::from_filepath(&filepath)
                .expect_err("Successfully loaded workspace from filepath");
            assert_eq!(error, WorkspaceLoadError::BadFileRead);
        }

        #[test]
        fn unexpected_format_error() {
            let mut temp_file =
                tempfile::NamedTempFile::new().expect("Could not get temporary file");
            temp_file
                .write(b"junkdata")
                .expect("Could not write to temporary file");

            let filepath = temp_file.path();
            let error = Workspace::from_filepath(filepath)
                .expect_err("Successfully loaded workspace from filepath");
            assert_eq!(error, WorkspaceLoadError::UnexpectedFormat);
        }
    }

    mod from_name {

        use super::*;

        #[test]
        #[serial_test::serial]
        fn success() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let filename = "testws";

            let mut workspace_filepath = get_workspace_dir().join(filename);
            workspace_filepath.set_extension("json");

            let test_filepath = PathBuf::from("tests/assets/workspaces/testws.json");
            fs::File::create_new(&workspace_filepath)
                .expect("Could not create new mock workspace file");
            fs::copy(&test_filepath, &workspace_filepath)
                .expect("Could not copy the mock workspace file");

            let _workspace: Workspace =
                Workspace::from_name(&filename).expect("Could not retrieve the workspace");

            crate::test_support::restore_previous_state(preexisted);
        }

        #[test]
        #[serial_test::serial]
        fn error() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let filename = "doesnotexist";

            let error =
                Workspace::from_name(&filename).expect_err("Successfully retrieved the workspace");
            assert_eq!(error, WorkspaceLoadError::DoesNotExist);

            crate::test_support::restore_previous_state(preexisted);
        }
    }

    mod save_as_filepath {

        use super::*;

        #[test]
        fn success() {
            let load_filepath = PathBuf::from("tests/assets/workspaces/testws.json");
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            let tempdir = TempDir::new().expect("Could not get temporary directory");

            let save_filepath = tempdir.path().join("testsave");
            workspace
                .save_as_filepath(&save_filepath)
                .expect("Could not save the workspace");

            let loaded = fs::read_to_string(&load_filepath)
                .expect("Could not load contents of test workspace");
            let saved = fs::read_to_string(&save_filepath)
                .expect("Could not load contents of saved workspace");

            assert_eq!(saved.trim(), loaded.trim());
        }

        #[test]
        fn bad_file_save() {
            let workspace: Workspace = Workspace::new("desc", &[]);

            let filepath = PathBuf::from("/does/not/exist");
            let error = workspace
                .save_as_filepath(&filepath)
                .expect_err("Successfully saved the workspace");

            assert_eq!(error, WorkspaceSaveError::BadFileSave);
        }
    }

    mod save_as_name {

        use super::*;

        #[test]
        #[serial_test::serial]
        fn new() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let filename = "testws";

            let load_filepath = PathBuf::from("tests/assets/workspaces/testws.json");
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            let mut save_filepath = get_workspace_dir().join("testws");
            save_filepath.set_extension("json");

            workspace
                .save_as_name(&filename, false)
                .expect("Could not save workspace");

            let loaded = fs::read_to_string(&load_filepath)
                .expect("Could not load contents of test workspace");
            let saved = fs::read_to_string(&save_filepath)
                .expect("Could not load contents of saved workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(saved.trim(), loaded.trim());
        }

        #[test]
        #[serial_test::serial]
        fn existing() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let filename = "testws";

            let load_filepath = PathBuf::from("tests/assets/workspaces/testws.json");
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            let mut save_filepath = get_workspace_dir().join("testws");
            save_filepath.set_extension("json");

            fs::File::create_new(&save_filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &save_filepath).expect("Could not copy file contents");

            workspace
                .save_as_name(&filename, true)
                .expect("Could not save workspace");

            let loaded = fs::read_to_string(&load_filepath)
                .expect("Could not load contents of test workspace");
            let saved = fs::read_to_string(&save_filepath)
                .expect("Could not load contents of saved workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(saved.trim(), loaded.trim());
        }

        #[test]
        #[serial_test::serial]
        fn already_exists_error() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let filename = "testws";

            let load_filepath = PathBuf::from("tests/assets/workspaces/testws.json");
            let workspace: Workspace = Workspace::from_filepath(&load_filepath)
                .expect("Could not get workspace from filepath");

            let mut save_filepath = get_workspace_dir().join("testws");
            save_filepath.set_extension("json");

            fs::File::create_new(&save_filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &save_filepath).expect("Could not copy file contents");

            let error = workspace
                .save_as_name(&filename, false)
                .expect_err("Successfully saved existing workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(error, WorkspaceSaveError::AlreadyExists);
        }
    }

    mod list_workspaces {

        use super::*;

        #[test]
        #[serial_test::serial]
        fn list_all() {
            let preexisted = crate::test_support::prepare_fresh_state();

            for i in 0..3 {
                let filepath = get_workspace_dir().join(format!("test{i}.json"));
                fs::File::create_new(&filepath).expect("Could not create new file");
            }

            let ignored_directory = get_workspace_dir().join("junkfolder");
            fs::create_dir(&ignored_directory).expect("Could not create the directory");

            let expected = "test0\ntest1\ntest2";

            let response = list_workspaces().expect("Could not get the list of workspaces");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(&response, expected);
        }

        #[test]
        #[serial_test::serial]
        fn none_saved() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let expected = "No workspaces have been saved";

            let response = list_workspaces().expect("Could not get the list of workspaces");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(&response, expected);
        }
    }

    mod rename_workspace {

        use super::*;

        #[test]
        #[serial_test::serial]
        fn success() {
            let preexisted = crate::test_support::prepare_fresh_state();

            for i in 0..3 {
                let filepath = get_workspace_dir().join(format!("test{i}.json"));
                fs::File::create_new(&filepath).expect("Could not create new file");
            }

            let orig_name = "test1";
            let new_name = "test55";

            let mut orig_filepath = get_workspace_dir().join(&orig_name);
            orig_filepath.set_extension("json");

            let mut new_filepath = get_workspace_dir().join(&new_name);
            new_filepath.set_extension("json");

            let orig_contents =
                fs::read_to_string(&orig_filepath).expect("Could not read file contents");

            let expected = format!("Renamed workspace '{orig_name}' to '{new_name}'");

            let response =
                rename_workspace(&orig_name, &new_name).expect("Could not rename workspace");

            let new_contents =
                fs::read_to_string(&new_filepath).expect("Could not read file contents");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(response, expected);
            assert_eq!(orig_contents, new_contents);
        }

        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let name = "doesnotexist";

            let expected = format!("Workspace '{name}' does not exist");

            let response = rename_workspace(&name, "newname")
                .expect_err("Successfully renamed nonexistent workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(response, expected);
        }
    }

    mod delete_workspace {

        use super::*;

        #[test]
        #[serial_test::serial]
        fn success() {
            let preexisted = crate::test_support::prepare_fresh_state();

            for i in 0..3 {
                let filepath = get_workspace_dir().join(format!("test{i}.json"));
                fs::File::create_new(&filepath).expect("Could not create new file");
            }

            let name = "test1";
            let filepath = Workspace::get_filepath_for_name(&name);

            let expected = format!("Deleted workspace '{name}'");
            let response = delete_workspace("test1").expect("Could not delete workspace");

            assert!(!filepath.exists());

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(expected, response);
        }

        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            let name = "doesnotexist";

            let expected = format!("Workspace '{name}' does not exist");
            let response =
                delete_workspace(name).expect_err("Successfully deleted nonexistent workspace");

            assert_eq!(expected, response);
        }
    }

    mod view_workspace {

        use super::*;

        #[test]
        #[serial_test::serial]
        fn success_with_description() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let load_filepath = PathBuf::from("tests/assets/workspaces/testws.json");

            let name = "withdescription";
            let filepath = Workspace::get_filepath_for_name(&name);

            fs::File::create_new(&filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &filepath).expect("Could not copy file");

            let base_filepath = PathBuf::from("/circpush");
            let write_filepath = PathBuf::from("/circpush/tests/assets/sandbox/");
            let path_components = [(base_filepath, write_filepath)];

            let expected = crate::test_support::generate_expected_parts(
                &path_components,
                0,
                Some(&format!("{name} - A test workspace")),
            );

            let response = view_workspace(name, true).expect("Could not view workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(
                crate::test_support::parse_contents(&response, true),
                expected
            );
        }

        #[test]
        #[serial_test::serial]
        fn success_without_description() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let load_filepath = PathBuf::from("tests/assets/workspaces/nodescription.json");

            let name = "nodescription";
            let filepath = Workspace::get_filepath_for_name(&name);

            fs::File::create_new(&filepath).expect("Could not create new file");
            fs::copy(&load_filepath, &filepath).expect("Could not copy file");

            let base_filepath = PathBuf::from("/circpush");
            let write_filepath = PathBuf::from("/circpush/tests/assets/sandbox/");
            let path_components = [(base_filepath, write_filepath)];

            let expected =
                crate::test_support::generate_expected_parts(&path_components, 0, Some(name));

            let response = view_workspace(name, true).expect("Could not view workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(
                crate::test_support::parse_contents(&response, true),
                expected
            );
        }

        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let name = "doesnotexist";

            let expected = format!("Workspace '{name}' does not exist");
            let response =
                view_workspace(name, true).expect_err("Successfully viewed nonexistent workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(response, expected);
        }

        #[test]
        #[serial_test::serial]
        fn unexpected_format_error() {
            let preexisted = crate::test_support::prepare_fresh_state();

            let name = "badformat";
            let filepath = Workspace::get_filepath_for_name(&name);

            fs::File::create_new(&filepath).expect("Could not create new file");

            let expected = format!("Could not parse the format of workspace '{name}'");
            let response = view_workspace(name, true)
                .expect_err("Successfully viewed incorrectly formatted workspace");

            crate::test_support::restore_previous_state(preexisted);

            assert_eq!(response, expected);
        }
    }
}
