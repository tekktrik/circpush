use crate::link::FileLink;
use glob::glob;
use pathdiff::diff_paths;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    hash::Hash,
    path::{absolute, Path, PathBuf},
};
use tabled::{builder::Builder, Table};

/// File monitor update errors
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateError {
    PartialGlobMatch,
    FileIOError,
    // BadFileLink,
}

/// Path-specific errors
#[derive(Debug, PartialEq, Eq)]
pub enum PathError {
    NoRelative,
}

/// File monitor structure
///
/// Stores a glob pattern to watch for. the base directory from which that
/// glob pattern should apply, and the write directory where files should
/// be copied to as the source files are found and updated.
///
/// These can be serialized via JSON for communication via TCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMonitor {
    pub read_pattern: String,
    pub write_directory: PathBuf,
    pub base_directory: PathBuf,
    links: HashSet<FileLink>,
}

impl FileMonitor {
    /// Creates a new FileMonitor, given the glob pattern for sources, the base directory,
    /// and relative write directory, with an emptry set of monitored file links
    pub fn new(read_pattern: &str, write_directory: &Path, base_directory: &Path) -> Self {
        Self {
            read_pattern: read_pattern.to_string(),
            write_directory: write_directory.to_path_buf(),
            base_directory: base_directory.to_path_buf(),
            links: HashSet::new(),
        }
    }

    /// Gets the write path for a given filepath
    fn get_write_path(&self, filepath: &PathBuf) -> Result<PathBuf, PathError> {
        match diff_paths(filepath, &self.base_directory) {
            Some(relative_path) => {
                let joinpath = self.write_directory.join(relative_path);
                Ok(absolute(joinpath).expect("Could not create absolute write path"))
            }
            None => Err(PathError::NoRelative),
        }
    }

    /// Calculate the monitored source files, returning an error if the glob match fails
    pub fn calculate_monitored_files(&self) -> Result<HashSet<FileLink>, UpdateError> {
        // Get the glob pattern as an absolute path string, by joining the pattern with the base directory
        let abs_read_directory = self.base_directory.join(&self.read_pattern);
        let read_dir_str = abs_read_directory.to_str().expect("Invalid read directory");

        // Match the glob file found
        match glob(read_dir_str) {
            Ok(paths) => {
                // Create the new set of files to return
                let mut new_hashset = HashSet::new();

                // Iterate through the files matched by the glob pattern, create FileLinks for them, and insert those links into the hash set
                for read_path in paths
                    .map(|result| result.expect("Could not read all glob matches"))
                    .filter(|path| path.is_file())
                {
                    let abs_read_path =
                        absolute(&read_path).expect("Unable to create absolute path");
                    let abs_write_path = self
                        .get_write_path(&read_path)
                        .expect("Could not get write path wile iterating paths");
                    let filelink = FileLink::new(&abs_read_path, &abs_write_path)
                        .expect("Could not create new FileLink");
                    new_hashset.insert(filelink);
                }

                // Return the constructed hash set
                Ok(new_hashset)
            }
            Err(_) => Err(UpdateError::PartialGlobMatch),
        }
    }

    /// Updates the stored file links by re-calculating the tracked files currently
    /// existing and handing the differences from the previously stored links
    pub fn update_links(&mut self) -> Result<(), UpdateError> {
        // Re-calculates the tracked files
        let new_filelinks = self.calculate_monitored_files()?;

        // Handle files that should be deleted
        for removed_file in self.links.difference(&new_filelinks) {
            if removed_file.delete().is_err() {
                return Err(UpdateError::FileIOError);
            }
        }

        // Create a list of file links from the hash set
        let mut new_filelinks_vec = Vec::from_iter(new_filelinks);

        // For re-calculated files, if the destination is outdated, ensure the write path and then
        // update the destination.
        for new_filelink in &mut new_filelinks_vec {
            if new_filelink.is_outdated() {
                new_filelink
                    .ensure_writepath()
                    .expect("Could not ensure write path");
                new_filelink
                    .update()
                    .expect("Unable to update the file link");
            }
        }

        // Create the hash set from the newly updated list, and restore it to the FileMonitor
        let new_filelinks = HashSet::from_iter(new_filelinks_vec);
        self.links = new_filelinks;

        Ok(())
    }

    /// Creates a table record from the FileMonitor for use with tabled, using either relative
    /// or absolute paths
    pub fn to_table_record(&self, absolute: bool) -> Vec<String> {
        // Get the current path and use it to create relative paths for the base and write
        // directories sif requested
        let current_dir = env::current_dir().expect("Could not get current directory");
        let base_directory = if absolute {
            &self.base_directory
        } else {
            &diff_paths(&self.base_directory, &current_dir).unwrap()
        };
        let write_directory = if absolute {
            &self.write_directory
        } else {
            &diff_paths(&self.write_directory, &current_dir).unwrap()
        };

        // Convert blank strings to "." for printing
        let mut base_directory_str = base_directory
            .to_str()
            .expect("Could not convert base directory to String");
        let mut write_directory_str = write_directory
            .to_str()
            .expect("Could not convert write directory to String");
        if base_directory_str.is_empty() {
            base_directory_str = ".";
        }
        if write_directory_str.is_empty() {
            write_directory_str = ".";
        }

        // Return the list representation
        vec![
            self.read_pattern.to_owned(),
            String::from(base_directory_str),
            String::from(write_directory_str),
        ]
    }

    /// Creates a header for the FileMonitor for use with tabled
    pub fn table_header() -> Vec<&'static str> {
        vec![
            "Link #",
            "Read Pattern",
            "Base Directory",
            "Write Directory",
        ]
    }

    /// Checks whether the write directory exists
    pub fn write_directory_exists(&self) -> bool {
        self.write_directory.as_path().is_dir()
    }

    /// Get a linkless clone of the current file monitor
    pub fn clone_linkless(&self) -> Self {
        let mut linkless = self.clone();
        linkless.links.clear();
        linkless
    }
}

impl PartialEq for FileMonitor {
    fn eq(&self, other: &Self) -> bool {
        self.read_pattern == other.read_pattern
            && self.write_directory == other.write_directory
            && self.base_directory == other.base_directory
    }
}

impl Eq for FileMonitor {}

impl Hash for FileMonitor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.read_pattern.hash(state);
        self.write_directory.hash(state);
        self.base_directory.hash(state);
    }
}

pub fn as_table(monitors: &[FileMonitor], number: usize, absolute: bool) -> Table {
    // Create a tabled table to be built and add the header row
    let mut table_builder = Builder::default();
    table_builder.push_record(FileMonitor::table_header());

    // For each FileMonitor returned, get the associated table record and add it along with the associated monitor number
    for (index, monitor) in monitors.iter().enumerate() {
        let mut record = monitor.to_table_record(absolute);
        let record_number = if number == 0 { index + 1 } else { number };
        record.insert(0, record_number.to_string());
        table_builder.push_record(record);
    }

    // Return a built table
    table_builder.build()
}

#[cfg(test)]
mod tests {

    use super::*;

    use tempfile::TempDir;

    mod filemonitor {

        use std::fs;

        use super::*;

        /// Creates a new file monitor for a tests, with created temporary read and write directory
        fn get_monitor() -> (FileMonitor, TempDir, TempDir) {
            // Create a new temporary directory for the read directory
            let read_directory =
                TempDir::new().expect("Could not get the temporary read directory");

            // Create four files in the read directory
            for i in 0..4 {
                let filename = format!("test_file{i}");
                fs::File::create_new(read_directory.path().join(&filename))
                    .expect(&format!("Could not create {filename}"));
            }

            // Create a new temporary for the write directory
            let write_directory =
                TempDir::new().expect("Could not get the temporary write directory");

            // STore the read pattern for the file monitor
            let read_pattern = "test*";

            // Create the file monitor
            let monitor = FileMonitor {
                read_pattern: read_pattern.to_string(),
                write_directory: write_directory.path().to_path_buf(),
                base_directory: read_directory.path().to_path_buf(),
                links: HashSet::new(),
            };

            // Return the file monitor and temporary read and write directories
            (monitor, read_directory, write_directory)
        }

        /// Tests FileMonitor::new()
        #[test]
        fn new() {
            // Create a new file monitor
            let read_pattern = "test_file";
            let write_directory = TempDir::new().expect("Could not get temporary directory");
            let base_directory = TempDir::new().expect("Could not get temporary directory");
            let monitor =
                FileMonitor::new(&read_pattern, write_directory.path(), base_directory.path());

            // Check the fields of the file monitor
            assert_eq!(monitor.read_pattern, read_pattern);
            assert_eq!(monitor.write_directory, write_directory.into_path());
            assert_eq!(monitor.base_directory, base_directory.into_path());
            assert!(monitor.links.is_empty());
        }

        mod get_write_path {

            use super::*;

            /// Tests the successful use of FileMonitor::get_write_path()
            #[test]
            fn success() {
                // Generate a file monitor
                let (monitor, read_dir, write_dir) = get_monitor();

                // Get a filepath for a hypohetical file in the read directory
                let filename = "test_file1";
                let filepath = read_dir.path().join(&filename);

                // Get the write path for the hypothetical file
                let write_path = monitor
                    .get_write_path(&filepath)
                    .expect("Could not get write path for the file");

                // Calculate the intended write path for the hypothetical file
                let intended_path = write_dir.path().join(&filename);

                // Check the write paths are the same
                assert_eq!(write_path, intended_path);
            }

            /// Tests the unsuccessful use of FileMonitor::get_write_path()
            #[test]
            fn error() {
                // Generate a file monitor
                let (monitor, _read_dir, _write_dir) = get_monitor();

                // Create a hypothetical relative filepath
                let relative_path = PathBuf::from("../relative_path");

                // Check that getting write path of the hypothetical relative fileoath is an error
                let error = monitor.get_write_path(&relative_path).expect_err(
                    "Successfully calculated write path when it should have been impossible",
                );
                assert_eq!(error, PathError::NoRelative);
            }
        }

        mod calculate_monitored_files {

            use super::*;

            /// Tests the successful use case of FileMonitor::calculate_monitored_files()
            #[test]
            fn success() {
                // Generate a file monitor
                let (monitor, read_dir, write_dir) = get_monitor();

                // Calculate the files to be monitored
                let files = monitor
                    .calculate_monitored_files()
                    .expect("Could not calculate the monitored files");

                // Check that four files should be monitored
                assert_eq!(files.len(), 4);

                // For each file index
                for i in 0..4 {
                    // Get the read and write filepaths of a monitored file
                    let filename = format!("test_file{i}");
                    let read_filepath = read_dir.path().join(&filename);
                    let write_filepath = write_dir.path().join(&filename);

                    // Check that the corresponding file link is contained in the files claculated
                    let link = FileLink::new(&read_filepath, &write_filepath)
                        .expect("Could not create file link");
                    assert!(files.contains(&link));
                }
            }

            /// Tests the unsuccessful use case of FileMonitor::calculate_monitored_files()
            #[test]
            fn error() {
                // Generate a file monitor
                let (mut monitor, _read_dir, _write_dir) = get_monitor();

                // Set the file monitor read pattern to a bad regex
                monitor.read_pattern = String::from("text[text");

                // Check that calculating the monitored files causes an error
                let error = monitor
                    .calculate_monitored_files()
                    .expect_err("Matched bad glob pattern");
                assert_eq!(error, UpdateError::PartialGlobMatch);
            }
        }

        mod update_links {

            use super::*;

            use filetime::{set_file_mtime, FileTime};

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A tracked file is modified
            #[test]
            fn modification() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file0";
                let read_path = read_dir.path().join(&filename);
                let write_path = write_dir.path().join(&filename);

                // Write test data to the read filepath
                let contents = "updated";
                fs::write(&read_path, contents).expect("Could not write to the first file");

                // Check that the read file exists and the write file does not exist yet
                assert!(read_path.as_path().is_file());
                assert!(!write_path.as_path().exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check that the read file still exists and the write file now exists
                assert!(read_path.as_path().is_file());
                assert!(write_path.as_path().is_file());

                // Check that the contents of the updated file written to match the expected contents
                let updated =
                    fs::read_to_string(&write_path).expect("Could not read the first test file");
                assert_eq!(&updated, contents);
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A file that would be tracked is deleted before updating
            #[test]
            fn predeletion() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file1";
                let read_path = read_dir.path().join(&filename);
                let write_path = write_dir.path().join(&filename);

                // Remove the read file
                fs::remove_file(&read_path).expect("Could not delete the second test file");

                // Check that neither the read nor write file exists
                assert!(!read_path.as_path().exists());
                assert!(!write_path.as_path().exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check that that read and write files still don't exist
                assert!(!read_path.as_path().exists());
                assert!(!write_path.as_path().exists());
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A tracked file is updated before being updated
            #[test]
            fn prewrite() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file2";
                let read_path = read_dir.path().join(&filename);
                let write_path = write_dir.path().join(&filename);

                // Write to the writefile ahead of updateing (which also updates its modification time)
                let write_contents2 = "testdata";
                fs::write(&write_path, write_contents2)
                    .expect("Could not write to the third test file");

                // Check that the read and write files both exist
                assert!(read_path.as_path().is_file());
                assert!(write_path.as_path().is_file());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check that the read and write files still exist
                assert!(read_path.as_path().is_file());
                assert!(write_path.as_path().is_file());

                // CHeck that the write file contents have not changed
                let updated =
                    fs::read_to_string(&write_path).expect("Could not read the thrid test file");
                assert_eq!(&updated, write_contents2);
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A newly created file yet to be tracked is updated
            #[test]
            fn new() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file3";
                let read_path = read_dir.path().join(&filename);
                let write_path = write_dir.path().join(&filename);

                // Check that only the read file exists
                assert!(read_path.as_path().is_file());
                assert!(!write_path.as_path().exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check that both the read and write files exist
                assert!(read_path.as_path().is_file());
                assert!(write_path.as_path().is_file());
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A newly created and written to file yet to be tracked is updated
            #[test]
            fn new_and_modified() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file4";
                let read_path = read_dir.path().join(&filename);
                let write_path = write_dir.path().join(&filename);

                // Write to the read file before updating
                let contents = "newdata";
                fs::write(&read_path, contents)
                    .expect("Could not create and write to the fifth test file");

                // Check that only the read file exists
                assert!(read_path.as_path().is_file());
                assert!(!write_path.as_path().exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check that both the read and write files exist
                assert!(read_path.as_path().is_file());
                assert!(write_path.as_path().is_file());

                // Check that the contents of the write file match what was written
                let updated =
                    fs::read_to_string(&write_path).expect("Could not read the fifth test file");
                assert_eq!(&updated, contents);
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - The modification time of the source is updated
            #[test]
            fn updated_mtime() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file5";
                let read_path = read_dir.path().join(&filename);
                let write_path = write_dir.path().join(&filename);

                // Get different contents to write for each file
                let write_contents = "oldtext";
                let contents = "newtext";
                assert_ne!(write_contents, contents);

                // Write each of the contents to the read and write files
                fs::write(&write_path, &write_contents)
                    .expect("Could not write to the write directory for the sixth file");
                fs::write(&read_path, &contents)
                    .expect("Could not write to the read directory for the sixth file");

                // Set the modification time of the write file to before the read file (read updated after write)
                set_file_mtime(&write_path, FileTime::from_unix_time(0, 0))
                    .expect("Could not set file modification time");

                // Check that both the read and write files exist
                assert!(read_path.as_path().is_file());
                assert!(write_path.as_path().is_file());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check that both the read and write files still exist
                assert!(read_path.as_path().is_file());
                assert!(write_path.as_path().is_file());

                // CHeck that the write file contents now match those of the read file
                let updated =
                    fs::read_to_string(&write_path).expect("Could not read the sixth test file");
                assert_eq!(&updated, contents);
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A deletion of the source file results in the deletion of the destination file
            #[test]
            fn deletion() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file1";
                let read_path = read_dir.path().join(&filename);
                let write_path = write_dir.path().join(&filename);

                // Create the new write path file
                fs::File::create_new(&write_path).expect("Could not create file");

                // Insert the new file link as a monitored link
                let link =
                    FileLink::new(&read_path, &write_path).expect("Could not create file link");
                monitor.links.insert(link);

                // Delete the existing source file
                fs::remove_file(&read_path).expect("Could not delete filed");

                // Update the links
                monitor
                    .update_links()
                    .expect("Unable to delete file as part of update");

                // Check that the write path no longer exists
                assert!(!write_path.as_path().exists());
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A source file deletion results in trying to delete a nonexistent destination file
            #[test]
            fn file_io_error() {
                // Generate a file monitor
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Get the read and write paths for the test file
                let filename = "test_file4";
                let read_file = read_dir.path().join(&filename);
                let write_file = write_dir.path().join(&filename);

                // Create the new read path file
                fs::File::create_new(&read_file).expect("Could not create file");

                // Create a new file link for the read path
                let link =
                    FileLink::new(&read_file, &write_file).expect("Could not create file link");

                // Delete the created read path file
                fs::remove_file(&read_file).expect("Could not delete file");

                // Insert the file link as a monitored link
                monitor.links.insert(link);

                // Check that updating the links causes an error
                let error = monitor
                    .update_links()
                    .expect_err("Successfully updated broken link");
                assert_eq!(error, UpdateError::FileIOError);
            }

            /// Tests FileMonitor::update_links(), where:
            ///
            /// - A bad glob pattern is used for the read pattern
            #[test]
            fn glob_error() {
                // Generate a file monitor
                let (mut monitor, _read_dir, _write_dir) = get_monitor();

                // Set the read pattern to a bad glob pattern
                monitor.read_pattern = "text[text".to_string();

                // Check that updating the links causes an error
                let error = monitor
                    .update_links()
                    .expect_err("Matched bad glob pattern");
                assert_eq!(error, UpdateError::PartialGlobMatch);
            }
        }

        mod to_table_record {

            use super::*;

            /// Tests getting the file montior as a table record, where:
            ///
            /// - The paths are requested as absolute
            #[test]
            fn absolute() {
                // Generate a file monitor
                let (monitor, _read_dir, _write_dir) = get_monitor();

                // Get the file monitor as a table record
                let table = monitor.to_table_record(true);

                // Calculate the expected table record
                let read_pattern = monitor.read_pattern;
                let write_directory = monitor.write_directory.to_str().unwrap().to_string();
                let base_directory = monitor.base_directory.to_str().unwrap().to_string();
                let expected = vec![read_pattern, base_directory, write_directory];

                // Check that both the generated and calculated table record match
                assert_eq!(table, expected);
            }

            /// Tests getting the file montior as a table record, where:
            ///
            /// - The paths are requested as relative to the current working directory
            #[test]
            #[serial_test::serial]
            fn relative() {
                // Generate a file monitor
                let (monitor, _read_dir, _write_dir) = get_monitor();

                // Get the file monitor as a table record
                let table = monitor.to_table_record(false);

                // Calculate the expected table record
                let read_pattern = monitor.read_pattern;
                let current_dir = env::current_dir().expect("Could not get the current directory");
                let base_directory = diff_paths(&monitor.base_directory, &current_dir)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                let write_directory = diff_paths(&monitor.write_directory, &current_dir)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                let expected = vec![read_pattern, base_directory, write_directory];

                // Check that both the generated and calculated table record match
                assert_eq!(table, expected);
            }

            /// Tests getting the file montior as a table record, where:
            ///
            /// - The paths are requested as relative to the base directory
            #[test]
            #[serial_test::serial]
            fn relative_to_base() {
                // Generate a file monitor
                let (monitor, _read_dir, _write_dir) = get_monitor();

                // Store the current working directory path
                let current_dir = env::current_dir().expect("Could not get the current directory");

                // Set the working directory to the base directory of the file monitor
                env::set_current_dir(&monitor.base_directory)
                    .expect("Could not set the current directory for the test");

                // Get the file monitor as a table record
                let table = monitor.to_table_record(false);

                // Calculate the expected table record
                let read_pattern = monitor.read_pattern;
                let base_directory = String::from(".");
                let write_directory =
                    diff_paths(&monitor.write_directory, &env::current_dir().unwrap())
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                let expected = vec![read_pattern, base_directory, write_directory];

                // Check that both the generated and calculated table record match
                assert_eq!(table, expected);

                // Reset the working directory
                env::set_current_dir(&current_dir)
                    .expect("Could not reset the current directory for the test");
                assert_eq!(env::current_dir().unwrap(), current_dir);
            }

            /// Tests getting the file montior as a table record, where:
            ///
            /// - The paths are requested as relative to the write directory
            #[test]
            #[serial_test::serial]
            fn relative_to_write() {
                // Generate a file montior
                let (monitor, _read_dir, _write_dir) = get_monitor();

                // Store the current working directory path
                let current_dir = env::current_dir().expect("Could not get the current directory");

                // Set the working directory to the base directory of the file monitor
                env::set_current_dir(&monitor.write_directory)
                    .expect("Could not set the current directory for the test");

                // Get the file monitor as a table record
                let table = monitor.to_table_record(false);

                // Calculate the expected table record
                let read_pattern = monitor.read_pattern;
                let base_directory =
                    diff_paths(&monitor.base_directory, &env::current_dir().unwrap())
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                let write_directory = String::from(".");
                let expected = vec![read_pattern, base_directory, write_directory];

                // Check that both the generated and calculated table record match
                assert_eq!(table, expected);

                // Reset the working directory
                env::set_current_dir(&current_dir)
                    .expect("Could not reset the current directory for the test");
                assert_eq!(env::current_dir().unwrap(), current_dir);
            }
        }

        /// Tests FileMonitor::table_header()
        #[test]
        fn table_header() {
            let header = FileMonitor::table_header();
            let intended = vec![
                "Link #",
                "Read Pattern",
                "Base Directory",
                "Write Directory",
            ];
            assert_eq!(header, intended);
        }

        /// Tests FileMonitor::write_directory_exists()
        #[test]
        fn write_directory_exists() {
            // Generate a file monitor with an existing write directory
            let (mut monitor, _read_dir, _write_dir) = get_monitor();

            // Check that the write directory exists
            assert!(monitor.write_directory_exists());

            // Set the write directory to a nonexistent file
            monitor.write_directory = PathBuf::from("/does/not/exist");

            // Check that the write directory does not exist
            assert!(!monitor.write_directory_exists());
        }

        /// Tests FileMonitor::clone_linkless()
        #[test]
        fn clone_linkless() {
            // Generate a file monitor
            let (mut monitor, _readdir, _writedir) = get_monitor();

            // Clone the file monitor linkless
            let linkless = monitor.clone_linkless();

            // Clear the links from the original file monitor
            monitor.links.clear();

            // Check that both file monitors are still equal
            assert_eq!(linkless, monitor);
        }

        mod partial_eq {

            use super::*;

            /// Tests file monitor equality, where:
            ///
            /// - The file monitors are identical
            #[test]
            fn identical() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let monitor1 = monitor0.clone();
                assert_eq!(monitor0, monitor1);
            }

            /// Tests file monitor equality, where:
            ///
            /// - The only difference is the files monitored
            #[test]
            fn diff_links() {
                // Generate a file monitor
                let (monitor0, read_dir, write_dir) = get_monitor();

                // Clone the monitor
                let mut monitor1 = monitor0.clone();

                // Get the read and write filepaths for a new file link
                let filename = "test_file4";
                let read_file = read_dir.path().join(&filename);
                let write_file = write_dir.path().join(&filename);

                // Create a new source file
                fs::File::create_new(&read_file).expect("Could not create file");

                // Insert a file link into the list of tracked links of the cloned monitor
                let link =
                    FileLink::new(&read_file, &write_file).expect("Could not create file link");
                monitor1.links.insert(link);

                // Check that the file monitors are still equal
                assert_eq!(monitor0, monitor1);
            }

            /// Tests file monitor equality, where:
            ///
            /// - The only difference is the read patterns
            #[test]
            fn diff_read_pattern() {
                // Generate a file monitor
                let (monitor0, _read_dir, _write_dir) = get_monitor();

                // Clone the file monitor
                let mut monitor1 = monitor0.clone();

                // Change the read pattern for the cloned file monitor
                monitor1.read_pattern = String::from("different");

                // Check that the file monitors are no longer equal
                assert_ne!(monitor0, monitor1);
            }

            /// Tests file monitor equality, where:
            ///
            /// - The only difference is the base directories
            #[test]
            fn diff_base_directory() {
                // Generate a file monitor
                let (monitor0, _read_dir, _write_dir) = get_monitor();

                // Clone the file monitor
                let mut monitor1 = monitor0.clone();

                // Change the base directory for the cloned file monitor
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.base_directory = tempdir.path().to_path_buf();

                // Check that the file monitors are no longer equal
                assert_ne!(monitor0, monitor1);
            }

            /// Tests file monitor equality, where:
            ///
            /// - The only difference is the write directories
            #[test]
            fn diff_write_directory() {
                // Generate a file monitor
                let (monitor0, _read_dir, _write_dir) = get_monitor();

                // Clone the file monitor
                let mut monitor1 = monitor0.clone();

                // Change the write directory for the cloned file monitor
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.write_directory = tempdir.path().to_path_buf();

                // Check that the file monitors are no longer equal
                assert_ne!(monitor0, monitor1);
            }
        }

        mod hash {

            use std::hash::{DefaultHasher, Hasher};

            use super::*;

            /// Tests file monitor hash equality, where:
            ///
            /// - The file monitors are identical
            #[test]
            fn identical() {
                // Generate a file monitor
                let (monitor0, _read_dir, _write_dir) = get_monitor();

                // Clone the file monitor
                let monitor1 = monitor0.clone();

                // Feed the first file monitor into its hasher
                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);

                // Feed the second file monitor into its hasher
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                // Assert the hash values for the file monitors are both equal
                assert_eq!(hasher0.finish(), hasher1.finish());
            }

            /// Tests file monitor hash equality, where:
            ///
            /// - The only difference is the files monitored
            #[test]
            fn diff_links() {
                // Generate a file monitor
                let (monitor0, read_dir, write_dir) = get_monitor();

                // Clone the file monitor
                let mut monitor1 = monitor0.clone();

                // Get the read and write filepaths for a new file link
                let filename = "test_file4";
                let read_file = read_dir.path().join(&filename);
                let write_file = write_dir.path().join(&filename);

                // Create a new source file
                fs::File::create_new(&read_file).expect("Could not create file");

                // Insert a file link into the list of tracked links of the cloned monitor
                let link =
                    FileLink::new(&read_file, &write_file).expect("Could not create file link");
                monitor1.links.insert(link);

                // Feed the first file monitor into its hasher
                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);

                // Feed the second file monitor into its hasher
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                // Check that the file monitors are still equal
                assert_eq!(hasher0.finish(), hasher1.finish());
            }

            /// Tests file monitor hash equality, where:
            ///
            /// - The only difference is the read patterns
            #[test]
            fn diff_read_pattern() {
                // Generate a file monitor
                let (monitor0, _read_dir, _write_dir) = get_monitor();

                // Clone the file monitor
                let mut monitor1 = monitor0.clone();

                // Change the read pattern for the cloned monitor
                monitor1.read_pattern = String::from("different");

                // Feed the first file monitor into its hasher
                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);

                // Feed the second file monitor into its hasher
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                // Check that the file monitors are no longer equal
                assert_ne!(hasher0.finish(), hasher1.finish());
            }

            /// Tests file monitor hash equality, where:
            ///
            /// - The only difference is the base directories
            #[test]
            fn diff_base_directory() {
                // Generate a file monitor
                let (monitor0, _read_dir, _write_dir) = get_monitor();

                // Clone the file monitor
                let mut monitor1 = monitor0.clone();

                // Change the base directory for the cloned file monitor
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.base_directory = tempdir.path().to_path_buf();

                // Feed the first file monitor into its hasher
                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);

                // Feed the second file monitor into its hasher
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                // Check that the file monitors are no longer equal
                assert_ne!(hasher0.finish(), hasher1.finish());
            }

            /// Tests file monitor hash equality, where:
            ///
            /// - The only difference is the write directories
            #[test]
            fn diff_write_directory() {
                // Generate a file monitor
                let (monitor0, _read_dir, _write_dir) = get_monitor();

                // Clone the file monitor
                let mut monitor1 = monitor0.clone();

                // Change the write directory for the cloned file monitor
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.write_directory = tempdir.path().to_path_buf();

                // Feed the first file monitor into its hasher
                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);

                // Feed the second file monitor into its hasher
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                // Check that the file monitors are no longer equal
                assert_ne!(hasher0.finish(), hasher1.finish());
            }
        }
    }
}
