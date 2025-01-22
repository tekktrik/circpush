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
        self.write_directory.exists()
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

        fn get_monitor() -> (FileMonitor, TempDir, TempDir) {
            let read_directory =
                TempDir::new().expect("Could not get the temporary read directory");
            for i in 0..4 {
                let filename = format!("test_file{i}");
                fs::File::create_new(read_directory.path().join(&filename))
                    .expect(&format!("Could not create {filename}"));
            }

            let write_directory =
                TempDir::new().expect("Could not get the temporary write directory");

            let read_pattern = "test*";

            let monitor = FileMonitor {
                read_pattern: read_pattern.to_string(),
                write_directory: write_directory.path().to_path_buf(),
                base_directory: read_directory.path().to_path_buf(),
                links: HashSet::new(),
            };

            (monitor, read_directory, write_directory)
        }

        #[test]
        fn new() {
            let read_pattern = "test_file";
            let write_directory = TempDir::new().expect("Could not get temporary directory");
            let base_directory = TempDir::new().expect("Could not get temporary directory");
            let monitor =
                FileMonitor::new(&read_pattern, write_directory.path(), base_directory.path());

            assert_eq!(monitor.read_pattern, read_pattern);
            assert_eq!(monitor.write_directory, write_directory.into_path());
            assert_eq!(monitor.base_directory, base_directory.into_path());
            assert!(monitor.links.is_empty());
        }

        mod get_write_path {

            use std::str::FromStr;

            use super::*;

            #[test]
            fn success() {
                let (monitor, read_dir, write_dir) = get_monitor();
                let filename = "test_file1";
                let filepath = read_dir.path().join(&filename);

                let write_path = monitor
                    .get_write_path(&filepath)
                    .expect("Could not get write path for the file");
                let intended_path = write_dir.path().join(&filename);

                assert_eq!(write_path, intended_path);
            }

            #[test]
            fn error() {
                let (monitor, _read_dir, _write_dir) = get_monitor();
                let relative_path = &PathBuf::from_str("relative_path")
                    .expect("Could not get a path for the test variable");
                let error = monitor.get_write_path(&relative_path).expect_err(
                    "Successfully calculated write path when it should have been impossible",
                );
                assert_eq!(error, PathError::NoRelative);
            }
        }

        mod calculate_monitored_files {

            use super::*;

            #[test]
            fn success() {
                let (monitor, read_dir, write_dir) = get_monitor();
                let files = monitor
                    .calculate_monitored_files()
                    .expect("Could not calculate the monitored files");

                assert_eq!(files.len(), 4);
                for i in 0..4 {
                    let filename = format!("test_file{i}");
                    let read_filepath = read_dir.path().join(&filename);
                    let write_filepath = write_dir.path().join(&filename);
                    let link = FileLink::new(&read_filepath, &write_filepath)
                        .expect("Could not create file link");
                    assert!(files.contains(&link));
                }
            }

            #[test]
            fn error() {
                let (mut monitor, _read_dir, _write_dir) = get_monitor();
                monitor.read_pattern = "text[text".to_string();

                let error = monitor
                    .calculate_monitored_files()
                    .expect_err("Matched bad glob pattern");
                assert_eq!(error, UpdateError::PartialGlobMatch);
            }
        }

        mod update_links {

            use super::*;

            use filetime::{set_file_mtime, FileTime};

            #[test]
            fn modification() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                let name0 = "test_file0";
                let read_path0 = read_dir.path().join(&name0);
                let write_path0 = write_dir.path().join(&name0);

                let contents0 = "updated";
                fs::write(&read_path0, contents0).expect("Could not write to the first file");

                assert!(read_path0.exists());
                assert!(!write_path0.exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                assert!(read_path0.exists());
                assert!(write_path0.exists());

                let updated0 =
                    fs::read_to_string(&read_path0).expect("Could not read the first test file");
                assert_eq!(&updated0, contents0);
            }

            #[test]
            fn predeletion() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                let name1 = "test_file1";
                let read_path1 = read_dir.path().join(&name1);
                let write_path1 = write_dir.path().join(&name1);

                fs::remove_file(&read_path1).expect("Could not delete the second test file");

                assert!(!read_path1.exists());
                assert!(!write_path1.exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check results of deleting the second test file
                assert!(!read_path1.exists());
                assert!(!write_path1.exists());
            }

            #[test]
            fn prewrite() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Pre-write the third file (which also updates its modification time)
                let name2 = "test_file2";
                let read_path2 = read_dir.path().join(&name2);
                let write_path2 = write_dir.path().join(&name2);

                let write_contents2 = "testdata";
                fs::write(&write_path2, write_contents2)
                    .expect("Could not write to the third test file");

                assert!(read_path2.exists());
                assert!(write_path2.exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check results of the pre-written third test file
                assert!(read_path2.exists());
                assert!(write_path2.exists());

                let updated2 =
                    fs::read_to_string(&write_path2).expect("Could not read the thrid test file");
                assert_eq!(&updated2, write_contents2);
            }

            #[test]
            fn new() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                let name3 = "test_file3";
                let read_path3 = read_dir.path().join(&name3);
                let write_path3 = write_dir.path().join(&name3);

                assert!(read_path3.exists());
                assert!(!write_path3.exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check results of updating the fourth test file
                assert!(read_path3.exists());
                assert!(write_path3.exists());
            }

            #[test]
            fn detection() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                let name4 = "test_file4";
                let read_path4 = read_dir.path().join(&name4);
                let write_path4 = write_dir.path().join(&name4);

                let contents4 = "newdata";
                fs::write(&read_path4, contents4)
                    .expect("Could not create and write to the fifth test file");

                assert!(read_path4.exists());
                assert!(!write_path4.exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check results of updating the fifth test file
                assert!(read_path4.exists());
                assert!(write_path4.exists());
                let updated4 =
                    fs::read_to_string(&write_path4).expect("Could not read the fifth test file");
                assert_eq!(&updated4, contents4);
            }

            #[test]
            fn updated_mtime() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                // Add a sixth file which will be updated
                let name5 = "test_file5";
                let read_path5 = read_dir.path().join(&name5);
                let write_path5 = write_dir.path().join(&name5);
                let write_contents5 = "oldtext";
                let contents5 = "newtext";
                assert_ne!(write_contents5, contents5);
                fs::write(&write_path5, &write_contents5)
                    .expect("Could not write to the write directory for the sixth file");
                fs::write(&read_path5, &contents5)
                    .expect("Could not write to the read directory for the sixth file");
                set_file_mtime(&write_path5, FileTime::from_unix_time(0, 0))
                    .expect("Could not set file modification time");
                assert!(read_path5.exists());
                assert!(write_path5.exists());

                // Update the links
                monitor.update_links().expect("Unable to update links");

                // Check results of updating the sixth test file
                assert!(read_path5.exists());
                assert!(write_path5.exists());
                let updated5 =
                    fs::read_to_string(&write_path5).expect("Could not read the sixth test file");
                assert_eq!(&updated5, contents5);
            }

            #[test]
            fn deletion() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                let name = "test_file1";
                let read_path = read_dir.path().join(&name);
                let write_path = write_dir.path().join(&name);

                fs::File::create_new(&write_path).expect("Could not create file");
                let link =
                    FileLink::new(&read_path, &write_path).expect("Could not create file link");
                monitor.links.insert(link);

                fs::remove_file(&read_path).expect("Could not delete filed");

                monitor
                    .update_links()
                    .expect("Unable to delete file as part of update");

                assert!(!write_path.exists());
            }

            #[test]
            fn file_io_error() {
                let (mut monitor, read_dir, write_dir) = get_monitor();

                let name = "test_file4";
                let read_file = read_dir.path().join(&name);
                let write_file = write_dir.path().join(&name);

                fs::File::create_new(&read_file).expect("Could not create file");
                let link =
                    FileLink::new(&read_file, &write_file).expect("Could not create file link");
                fs::remove_file(&read_file).expect("Could not delete file");
                monitor.links.insert(link);

                let error = monitor
                    .update_links()
                    .expect_err("Successfully updated broken link");
                assert_eq!(error, UpdateError::FileIOError);
            }

            #[test]
            fn glob_error() {
                let (mut monitor, _read_dir, _write_dir) = get_monitor();
                monitor.read_pattern = "text[text".to_string();

                let error = monitor
                    .update_links()
                    .expect_err("Matched bad glob pattern");
                assert_eq!(error, UpdateError::PartialGlobMatch);
            }
        }

        mod to_table_record {

            use super::*;

            #[test]
            fn absolute() {
                let (monitor, _read_dir, _write_dir) = get_monitor();
                let table = monitor.to_table_record(true);
                let read_pattern = monitor.read_pattern;
                let write_directory = monitor.write_directory.to_str().unwrap().to_string();
                let base_directory = monitor.base_directory.to_str().unwrap().to_string();
                let expected = vec![read_pattern, base_directory, write_directory];
                assert_eq!(table, expected);
            }

            #[test]
            #[serial_test::serial]
            fn relative() {
                let (monitor, _read_dir, _write_dir) = get_monitor();
                let table = monitor.to_table_record(false);
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
                assert_eq!(table, expected);
            }

            #[test]
            #[serial_test::serial]
            fn relative_to_base() {
                let (monitor, _read_dir, _write_dir) = get_monitor();
                let current_dir = env::current_dir().expect("Could not get the current directory");
                env::set_current_dir(&monitor.base_directory)
                    .expect("Could not set the current directory for the test");
                let table = monitor.to_table_record(false);
                let read_pattern = monitor.read_pattern;
                let base_directory = String::from(".");
                let write_directory =
                    diff_paths(&monitor.write_directory, &env::current_dir().unwrap())
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                let expected = vec![read_pattern, base_directory, write_directory];
                assert_eq!(table, expected);
                env::set_current_dir(&current_dir)
                    .expect("Could not reset the current directory for the test");
                assert_eq!(env::current_dir().unwrap(), current_dir);
            }

            #[test]
            #[serial_test::serial]
            fn relative_to_write() {
                let (monitor, _read_dir, _write_dir) = get_monitor();
                let current_dir = env::current_dir().expect("Could not get the current directory");
                env::set_current_dir(&monitor.write_directory)
                    .expect("Could not set the current directory for the test");
                let table = monitor.to_table_record(false);
                let read_pattern = monitor.read_pattern;
                let base_directory =
                    diff_paths(&monitor.base_directory, &env::current_dir().unwrap())
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                let write_directory = String::from(".");
                let expected = vec![read_pattern, base_directory, write_directory];
                assert_eq!(table, expected);
                env::set_current_dir(&current_dir)
                    .expect("Could not reset the current directory for the test");
                assert_eq!(env::current_dir().unwrap(), current_dir);
            }
        }

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

        #[test]
        fn write_directory_exists() {
            let (mut monitor, _read_dir, _write_dir) = get_monitor();
            assert!(monitor.write_directory_exists());

            monitor.write_directory = PathBuf::from("/does/not/exist");
            assert!(!monitor.write_directory_exists());
        }

        #[test]
        fn clone_linkless() {
            let (mut monitor, _readdir, _writedir) = get_monitor();

            let linkless = monitor.clone_linkless();

            monitor.links.clear();

            assert_eq!(linkless, monitor);
        }

        mod partial_eq {

            use super::*;

            #[test]
            fn identical() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let monitor1 = monitor0.clone();
                assert_eq!(monitor0, monitor1);
            }

            #[test]
            fn diff_links() {
                let (monitor0, read_dir, write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                let name = "test_file4";
                let read_file = read_dir.path().join(&name);
                let write_file = write_dir.path().join(&name);
                fs::File::create_new(&read_file).expect("Could not create file");
                let link =
                    FileLink::new(&read_file, &write_file).expect("Could not create file link");
                monitor1.links.insert(link);
                assert_eq!(monitor0, monitor1);
            }

            #[test]
            fn diff_read_pattern() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                monitor1.read_pattern = String::from("different");
                assert_ne!(monitor0, monitor1);
            }

            #[test]
            fn diff_base_directory() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.base_directory = tempdir.path().to_path_buf();
                assert_ne!(monitor0, monitor1);
            }

            #[test]
            fn diff_write_directory() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.write_directory = tempdir.path().to_path_buf();
                assert_ne!(monitor0, monitor1);
            }
        }

        mod hash {

            use std::hash::{DefaultHasher, Hasher};

            use super::*;

            #[test]
            fn identical() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let monitor1 = monitor0.clone();

                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                assert_eq!(hasher0.finish(), hasher1.finish());
            }

            #[test]
            fn diff_links() {
                let (monitor0, read_dir, write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                let name = "test_file4";
                let read_file = read_dir.path().join(&name);
                let write_file = write_dir.path().join(&name);
                fs::File::create_new(&read_file).expect("Could not create file");
                let link =
                    FileLink::new(&read_file, &write_file).expect("Could not create file link");
                monitor1.links.insert(link);

                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                assert_eq!(hasher0.finish(), hasher1.finish());
            }

            #[test]
            fn diff_read_pattern() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                monitor1.read_pattern = String::from("different");

                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                assert_ne!(hasher0.finish(), hasher1.finish());
            }

            #[test]
            fn diff_base_directory() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.base_directory = tempdir.path().to_path_buf();

                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                assert_ne!(hasher0.finish(), hasher1.finish());
            }

            #[test]
            fn diff_write_directory() {
                let (monitor0, _read_dir, _write_dir) = get_monitor();
                let mut monitor1 = monitor0.clone();
                let tempdir = TempDir::new().expect("Could not create new temporary directory");
                monitor1.write_directory = tempdir.path().to_path_buf();

                let mut hasher0 = DefaultHasher::new();
                monitor0.hash(&mut hasher0);
                let mut hasher1 = DefaultHasher::new();
                monitor1.hash(&mut hasher1);

                assert_ne!(hasher0.finish(), hasher1.finish());
            }
        }
    }
}
