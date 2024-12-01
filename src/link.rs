use std::borrow::Cow;
use std::fs::create_dir_all;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::fs;
use filetime::{FileTime, set_file_mtime};
use tabled::Tabled;
use serde::{Serialize, Deserialize};


fn get_file_mtime(path: &PathBuf) -> i64 {
    let metadata = fs::metadata(path).expect("Unable to retrieve file metadata");
    let modtime = FileTime::from_last_modification_time(&metadata);
    modtime.seconds()
}

// FileLink creation errors

#[derive(Debug)]
pub enum FileLinkCreationError {
    InvalidSource,
    InvalidDestination,
    DestinationSetup,
}

// UpdateError

#[derive(Debug)]
pub enum FileUpdateError {
    CopyFailed,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileLink {
    pub source: PathBuf,
    pub destination: PathBuf,
}

impl Tabled for FileLink {
    const LENGTH: usize = 2;

    fn fields(&self) -> Vec<std::borrow::Cow<'_, str>> {
        let source_str = self.source.to_str().expect("Could not convert source to string");
        let destination_str = self.destination.to_str().expect("Could not convert destination to string");
        vec![Cow::Borrowed(source_str), Cow::Borrowed(destination_str)]
    }

    fn headers() -> Vec<std::borrow::Cow<'static, str>> {
        vec![Cow::Borrowed("Source"), Cow::Borrowed("Destination")]
    }
}

impl FileLink {

    pub fn new(source: &Path, destination: &Path) -> Result<Self, FileLinkCreationError> {
        if !(source.is_file() && source.is_absolute()) {
            return Err(FileLinkCreationError::InvalidSource);
        }
        if !destination.is_absolute() {
            return Err(FileLinkCreationError::InvalidDestination);
        }

        let source_buf = source.to_path_buf();
        let destination_buf = destination.to_path_buf();
        let link = FileLink {
            source: source_buf,
            destination: destination_buf,
        };
        Ok(link)
    }

    pub fn ensure_writepath(&self)  -> Result<(), FileLinkCreationError>{
        if !self.destination.exists() {
            let dest_parent = self.destination.parent();
            match dest_parent {
                Some(parent_path) => {
                    if create_dir_all(parent_path).is_err() {
                        return Err(FileLinkCreationError::DestinationSetup);
                    }
                }
                None => return Err(FileLinkCreationError::InvalidDestination)
            }
        }
        Ok(())
    }
    
    pub fn is_outdated(&self) -> bool {
        if !self.destination.exists() { return true; }
        let source_mtime = get_file_mtime(&self.source);
        let destination_mtime = get_file_mtime(&self.destination);
            source_mtime > destination_mtime
    }

    pub fn update(&mut self) -> Result<u64, FileUpdateError> {
        // TODO: Wrong error returned
        let amount_copied = match fs::copy(&self.source, &self.destination) {
            Ok(amount_copied) => amount_copied,
            Err(_) => return Err(FileUpdateError::CopyFailed)
        };

        let mod_filetime = FileTime::now();
        set_file_mtime(&self.destination, mod_filetime).expect("Could not set destination file modification time");
        Ok(amount_copied)
    }

    pub fn delete(&self) -> std::io::Result<()> {
        fs::remove_file(&self.destination)
    }

    pub fn source_exists(&self) -> bool {
        self.source.exists()
    }

    pub fn destination_exists(&self) -> bool {
        self.destination.exists()
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
