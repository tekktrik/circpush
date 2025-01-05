use filetime::{set_file_mtime, FileTime};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fs;
use std::fs::create_dir_all;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use tabled::Tabled;

/// Get the modification time for a file given the filepath
fn get_file_mtime(path: &PathBuf) -> i64 {
    let metadata = fs::metadata(path).expect("Unable to retrieve file metadata");
    let modtime = FileTime::from_last_modification_time(&metadata);
    modtime.seconds()
}

/// FileLink creation errors
#[derive(Debug)]
pub enum FileLinkCreationError {
    InvalidSource,
    InvalidDestination,
    DestinationSetup,
}

// FileLink update errors
#[derive(Debug)]
pub enum FileUpdateError {
    CopyFailed,
}

/// File link structure for handling the connection between source
/// and destination filepaths
///
/// These can be serialized into JSON for communication via TCP
#[derive(Serialize, Deserialize, Clone)]
pub struct FileLink {
    source: PathBuf,
    destination: PathBuf,
}

impl FileLink {
    /// Create a new FileLink, given the source and destination filepaths
    ///
    /// The source path must be an existing file, and both the source and
    /// destination paths must be absolute.
    pub fn new(source: &Path, destination: &Path) -> Result<Self, FileLinkCreationError> {
        // If the source path is not an existing file or is not absolute, return an error
        if !(source.is_file() && source.is_absolute()) {
            return Err(FileLinkCreationError::InvalidSource);
        }

        // If the destination path is not absolute, return an error
        if !destination.is_absolute() {
            return Err(FileLinkCreationError::InvalidDestination);
        }

        // Convert the source and destinations into PathBuf
        let source_buf = source.to_path_buf();
        let destination_buf = destination.to_path_buf();

        // Create and return the FileLink
        let link = FileLink {
            source: source_buf,
            destination: destination_buf,
        };
        Ok(link)
    }

    /// Ensures that the write path directries exist, such that the source file can eventually be
    /// copied to the required destination
    pub fn ensure_writepath(&self) -> Result<(), FileLinkCreationError> {
        // Skip if the destination already exists
        if !self.destination.exists() {
            // Check the parent directory of the destination
            let dest_parent = self.destination.parent();
            match dest_parent {
                // The parent folder is valid
                Some(parent_path) => {
                    // Attempt to create all necessary directories, return an error if unsuccessful
                    if create_dir_all(parent_path).is_err() {
                        return Err(FileLinkCreationError::DestinationSetup);
                    }
                }
                // The parent folder is invalid
                None => return Err(FileLinkCreationError::InvalidDestination),
            }
        }
        Ok(())
    }

    /// Checks whether the destination file is outdated
    pub fn is_outdated(&self) -> bool {
        // If the destination file doesn't exist, it's outdated by definition
        if !self.destination.exists() {
            return true;
        }

        // Get the source and destination file modification times
        let source_mtime = get_file_mtime(&self.source);
        let destination_mtime = get_file_mtime(&self.destination);

        // Return where the source modification time is later than the destination modification time
        source_mtime > destination_mtime
    }

    /// Updates the file link, copying the source file to the destination
    ///
    /// Returns the number of bytes copied
    pub fn update(&mut self) -> Result<u64, FileUpdateError> {
        // Copy the source file contents to the destination file
        let amount_copied = match fs::copy(&self.source, &self.destination) {
            Ok(amount_copied) => amount_copied,
            Err(_) => return Err(FileUpdateError::CopyFailed),
        };

        // Set the destination file modification time to now
        let mod_filetime = FileTime::now();
        set_file_mtime(&self.destination, mod_filetime)
            .expect("Could not set destination file modification time");

        Ok(amount_copied)
    }

    /// Deletes the destination file
    pub fn delete(&self) -> std::io::Result<()> {
        fs::remove_file(&self.destination)
    }
}

impl PartialEq for FileLink {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.destination == other.destination
    }
}

impl Eq for FileLink {}

impl Hash for FileLink {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.destination.hash(state);
    }
}

impl Tabled for FileLink {
    /// The number of fields be displayed
    const LENGTH: usize = 2;

    /// How to print the fields of a FileLink for Tabled
    fn fields(&self) -> Vec<std::borrow::Cow<'_, str>> {
        let source_str = self
            .source
            .to_str()
            .expect("Could not convert source to string");
        let destination_str = self
            .destination
            .to_str()
            .expect("Could not convert destination to string");
        vec![Cow::Borrowed(source_str), Cow::Borrowed(destination_str)]
    }

    /// How to print the headers of a FileLink for Tabled
    fn headers() -> Vec<std::borrow::Cow<'static, str>> {
        vec![Cow::Borrowed("Source"), Cow::Borrowed("Destination")]
    }
}
