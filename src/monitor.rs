use crate::link::FileLink;
use glob::glob;
use pathdiff::diff_paths;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    hash::Hash,
    path::{absolute, PathBuf},
};

/// File monitor update errors
#[derive(Debug)]
pub enum UpdateError {
    PartialGlobMatch,
    FileIOError,
    BadFileLink,
}

/// Path-specific errors
#[derive(Debug)]
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
#[derive(Serialize, Deserialize)]
pub struct FileMonitor {
    read_pattern: String,
    write_directory: PathBuf,
    base_directory: PathBuf,
    links: HashSet<FileLink>,
}

impl FileMonitor {
    /// Creates a new FileMonitor, given the glob pattern for sources, the base directory,
    /// and relative write directory, with an emptry set of monitored file links
    pub fn new(
        read_pattern: String,
        write_directory: PathBuf,
        base_directory: PathBuf,
    ) -> Result<Self, PathError> {
        let file_monitor = Self {
            read_pattern,
            write_directory,
            base_directory,
            links: HashSet::new(),
        };
        Ok(file_monitor)
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

    // /// Iterate over the
    // fn iterate_paths(&self, paths: Paths) -> Result<HashSet<FileLink>, UpdateError> {
    //     let mut new_hashset = HashSet::new();
    //     for read_path in paths.map(|result| result.expect("Could not read all glob matches")).filter(|path| path.is_file()) {
    //         let abs_read_path = absolute(&read_path).expect("Unable to create absolute path");
    //         let abs_write_path = self.get_write_path(&read_path).expect("Could not get write path wile iterating paths");
    //         let filelink = FileLink::new(&abs_read_path, &abs_write_path).expect("Could not create new FileLink");
    //         new_hashset.insert(filelink);
    //     }
    //     Ok(new_hashset)
    // }

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
                if new_filelink.update().is_err() {
                    return Err(UpdateError::BadFileLink);
                }
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
