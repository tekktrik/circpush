use filetime::{set_file_mtime, FileTime};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fs;
use std::fs::create_dir_all;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use tabled::Tabled;

/// Get the modification time for a file given the filepath
fn get_file_mtime(path: &PathBuf) -> FileTime {
    let metadata = fs::metadata(path).expect("Unable to retrieve file metadata");
    FileTime::from_last_modification_time(&metadata) 
}

/// FileLink creation errors
#[derive(Debug, PartialEq, Eq)]
pub enum FileLinkCreationError {
    InvalidSource,
    InvalidDestination,
    DestinationSetup,
}

// FileLink update errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileUpdateError {
    CopyFailed,
}

/// File link structure for handling the connection between source
/// and destination filepaths
///
/// These can be serialized into JSON for communication via TCP
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
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
            // // Check the parent directory of the destination
            // let dest_parent = self.destination.parent();
            // match dest_parent {
            //     // The parent folder is valid
            //     Some(parent_path) => {
            //         // Attempt to create all necessary directories, return an error if unsuccessful
            //         if create_dir_all(parent_path).is_err() {
            //             return Err(FileLinkCreationError::DestinationSetup);
            //         }
            //     }
            //     // The parent folder is invalid
            //     None => return Err(FileLinkCreationError::InvalidDestination),
            // }

            // Check the parent directory of the destination
            let parent_path = self.destination.parent().expect("Could not get the parent of the destination");
            // Attempt to create all necessary directories, return an error if unsuccessful
            if create_dir_all(parent_path).is_err() {
                return Err(FileLinkCreationError::DestinationSetup);
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

        // Compare the source and destination file modification times
        let source_mtime = get_file_mtime(&self.source);
        let destination_mtime = get_file_mtime(&self.destination);
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
        // let mod_filetime = FileTime::now();
        let mod_filetime = get_file_mtime(&self.source);
        // let  = FileTime::from_unix_time(x.0, x.1)
        set_file_mtime(&self.destination, mod_filetime)
            .expect("Could not set destination file modification time");

        Ok(amount_copied)
    }

    /// Deletes the destination file
    pub fn delete(&self) -> std::io::Result<()> {
        fs::remove_file(&self.destination)
    }
}

// impl PartialEq for FileLink {
//     fn eq(&self, other: &Self) -> bool {
//         self.source == other.source && self.destination == other.destination
//     }
// }

// impl Eq for FileLink {}

// impl Hash for FileLink {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.source.hash(state);
//         self.destination.hash(state);
//     }
// }

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

#[cfg(test)]
mod tests {

    use super::*;

    use tempfile::{tempdir, NamedTempFile, TempDir};
    use std::path::absolute;

    fn create_new_filelink() -> (FileLink, NamedTempFile, NamedTempFile) {
        let srcfile = NamedTempFile::new().expect("Could not create temporary source file");
        let source = absolute(srcfile.path()).expect("Could not get absolute path of source");

        let destfile = NamedTempFile::new().expect("Could not open a temporary destination file");
        let destination = absolute(destfile.path()).expect("Could not get absolute path of destination file");

        let link = FileLink {
            source,
            destination,
        };

        (link, srcfile, destfile)
    }

    fn create_new_unwritten_filelink() -> (FileLink, NamedTempFile, TempDir) {
        let srcfile = NamedTempFile::new().expect("Could not create temporary source file");
        let source = absolute(srcfile.path()).expect("Could not get absolute path of source");

        let destdir = tempdir().expect("Could not open a temporary destination file");
        let destination = absolute(destdir.path()).expect("Could not get absolute path of destination file").join("testfile");

        let link = FileLink {
            source,
            destination,
        };

        (link, srcfile, destdir)
    }

    mod filelink {

        use super::*;

        mod new {

            use super::*;

            use std::env::current_dir;

            #[test]
            fn success() {
                // Test:
                // Source file that does exist (absolute path)
                // Destination file (absolute path)
                let srcfile = NamedTempFile::new().expect("Could not create temporary source file");
                let source = absolute(srcfile.path()).expect("Could not get absolute path of source");

                let destfile = NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path()).expect("Could not get absolute path of destination file");
                
                let _: FileLink = FileLink::new(&source, &destination).expect("Could not create a valid link");
            }

            #[test]
            fn source_does_not_exist() {
                // Test:
                // Source file that does not exist (absolute path)
                // Destination file (absolute path)
                let source = absolute("does/not/exist").expect("Could not get absolute path of source");

                let destfile = NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path()).expect("Could not get absolute path of destination file");

                let error = FileLink::new(&source, &destination).expect_err("Successfully created the file link when it should have been prevented");
                assert_eq!(error, FileLinkCreationError::InvalidSource);
            }

            #[test]
            fn source_directory() {
                // Test:
                // Source directory that does exist (absolute path)
                // Destination file (absolute path)
                let srcfile = tempdir().expect("Could not open a temporary source directory");
                let source = absolute(srcfile.path()).expect("Could not get absolute path of source");

                let destfile = NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path()).expect("Could not get absolute path of destination file");

                let error = FileLink::new(&source, &destination).expect_err("Successfully created the file link when it should have been prevented");
                assert_eq!(error, FileLinkCreationError::InvalidSource);
            }

            #[test]
            fn source_relative() {
                // Test:
                // Source file that does exist (relative path)
                // Destination file (absolute path)
                let current_dir = current_dir().expect("Could not get current directory");

                let srcfile = NamedTempFile::new().expect("Could not create temporary file");
                let source = pathdiff::diff_paths(&srcfile.path(), &current_dir).expect("Could not get relative path for source file");

                let destfile = NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path()).expect("Could not get absolute path of destination file");

                let error = FileLink::new(&source, &destination).expect_err("Successfully created the file link when it should have been prevented");
                assert_eq!(error, FileLinkCreationError::InvalidSource);
            }

            #[test]
            fn destination_relative() {
                // Test:
                // Source file that does exist (absolute path)
                // Destination file (relative path)
                let current_dir = current_dir().expect("Could not get current directory");

                let srcfile = NamedTempFile::new().expect("Could not create temporary file");
                let source = absolute(srcfile.path()).expect("Could not get absolute path of source");

                let destfile = NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = pathdiff::diff_paths(destfile.path(), &current_dir).expect("Could not get relative path for destination file");

                let error = FileLink::new(&source, &destination).expect_err("Successfully created the file link when it should have been prevented");
                assert_eq!(error, FileLinkCreationError::InvalidDestination);
            }
        }

        mod ensure_writepath {

            use super::*;

            #[test]
            fn destination_does_not_exist() {
                // Test
                // Successfully creates destination file
                let (mut filelink, _src, _dst) = create_new_unwritten_filelink();
                filelink.destination = filelink.destination.join("inner").join("newfile");
                assert!(!filelink.destination.parent().unwrap().exists());
                filelink.ensure_writepath().expect("Could not ensure file link destination");
                assert!(filelink.destination.parent().unwrap().exists());
            }

            #[test]
            fn destination_exists() {
                // Test
                // Does nothing as the file already exists
                let (filelink, _src, _dst) = create_new_filelink();
                assert!(filelink.destination.exists());
                filelink.ensure_writepath().expect("Could not ensure file link destination");
                assert!(filelink.destination.exists());
            }

            #[test]
            fn directory_creation_failure() {
                // Test
                // Fail to ensure write path because recursively creatings directories fails
                let (mut filelink, _src, _dst) = create_new_filelink();
                filelink.destination = filelink.source.join("testfile");
                assert!(filelink.destination.parent().unwrap().is_file());

                let error = filelink.ensure_writepath().expect_err("Successfully ensured an impossible destination");
                assert_eq!(error, FileLinkCreationError::DestinationSetup);

                assert!(filelink.destination.parent().unwrap().is_file());
                assert!(!filelink.destination.parent().unwrap().is_dir());
            }
        }

        mod is_outdated {

            use super::*;

            #[test]
            fn source_before_destination() {
                // Test
                // Destination file is outdated
                let (link, _src, _dst) = create_new_filelink();
                let orig_mtime = get_file_mtime(&link.source);
                let new_mtime = FileTime::from_unix_time(orig_mtime.unix_seconds() - 30, orig_mtime.nanoseconds());
                set_file_mtime(&link.destination, new_mtime).expect("Could not set modification time");
                assert!(link.is_outdated());
            }

            #[test]
            fn source_equals_destination() {
                // Test
                // Destination file is not outdated (it is equal to source)
                let (link, _src, _dst) = create_new_filelink();
                let orig_mtime = get_file_mtime(&link.source);
                set_file_mtime(&link.destination, orig_mtime).expect("Could not set modification time");
                assert!(!link.is_outdated());
            }

            #[test]
            fn source_after_destination() {
                // Test
                // Destination file is not outdated (it is after source)
                let (link, _src, _dst) = create_new_filelink();
                let orig_mtime = get_file_mtime(&link.source);
                let new_mtime = FileTime::from_unix_time(orig_mtime.unix_seconds() + 30, orig_mtime.nanoseconds());
                set_file_mtime(&link.destination, new_mtime).expect("Could not set modification time");
                assert!(!link.is_outdated());
            }

            #[test]
            fn desination_does_not_exist() {
                // Test
                // Destination file does not exist
                let (link, _src, _dst) = create_new_unwritten_filelink();
                assert!((link.is_outdated()))
            }
        }

        mod update {

            use std::io::Write;

            use super::*;

            #[test]
            fn success() {
                let (mut link, mut src, _dst) = create_new_filelink();
                
                let new_contents = b"test";
                src.write(new_contents).expect("Could not write to source file");

                let total: u64 = link.update().expect("Could not update file link");

                let src_contents = fs::read(&link.source).expect("Could not read source");
                let dst_contents = fs::read(&link.destination).expect("Could not read destination");

                assert_eq!(src_contents, dst_contents);
                assert_eq!(total, new_contents.len() as u64);
            }

            #[test]
            fn copy_failed() {
                let (mut link, mut src, _dst) = create_new_filelink();
                link.source = link.source.join("does/not/exist");
                
                let new_contents = b"test";
                src.write(new_contents).expect("Could not write to source file");

                let error = link.update().expect_err("Updated using non-existent source file");
                assert_eq!(error, FileUpdateError::CopyFailed);
            }
        }

        mod delete {

            use super::*;

            #[test]
            fn success() {
                let (link, _src, _dst) = create_new_unwritten_filelink();
                assert!(!link.destination.exists());

                fs::File::create(&link.destination).expect("Could not create new destination file");
                assert!(link.destination.exists());

                link.delete().expect("Could not delete file link destination file");
                assert!(!link.destination.exists());
            }
        }

        mod trait_tabled {

            use std::iter::zip;

            use super::*;

            #[test]
            fn fields() {
                let (link, src, dst) = create_new_filelink();

                let fields = link.fields();

                let source_path = src.path().to_str().expect("Could not get source path as string");
                let destination_path = dst.path().to_str().expect("Could not get destination path as string");
                let intendeds = vec![source_path, destination_path];

                for (field, intended) in zip(fields, intendeds) {
                    assert_eq!(field, intended);
                }
            }

            #[test]
            fn headers() {
                let headers = FileLink::headers();
                let intendeds = vec!["Source", "Destination"];
                for (header, intended) in zip(headers, intendeds) {
                    assert_eq!(header, intended);
                }
            }
        }
    }
}