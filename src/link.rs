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
        if !source.is_file() || !source.is_absolute() || source.is_symlink() {
            return Err(FileLinkCreationError::InvalidSource);
        }

        // If the destination path is not absolute, return an error
        if !destination.is_absolute() || destination.is_symlink() {
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

    /// Ensures that the write path directories exist, such that the source file can eventually be
    /// copied to the required destination
    pub fn ensure_writepath(&self) -> Result<(), FileLinkCreationError> {
        // Skip if the destination already exists
        if !self.destination.as_path().exists() {
            // Check the parent directory of the destination
            let parent_path = self
                .destination
                .parent()
                .expect("Could not get the parent of the destination");

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
        if !self.destination.as_path().exists() {
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
        let mod_filetime = get_file_mtime(&self.source);
        set_file_mtime(&self.destination, mod_filetime)
            .expect("Could not set destination file modification time");

        Ok(amount_copied)
    }

    /// Deletes the destination file
    pub fn delete(&self) -> std::io::Result<()> {
        fs::remove_file(&self.destination)
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

#[cfg(test)]
mod tests {

    use super::*;

    use std::path::absolute;
    use tempfile::{tempdir, NamedTempFile, TempDir};

    /// Creates a new file link for tests, with both a source and destination file created
    fn create_new_filelink() -> (FileLink, NamedTempFile, NamedTempFile) {
        // Get the absolute filepath to a temporary source file
        let srcfile = NamedTempFile::new().expect("Could not create temporary source file");
        let source = absolute(srcfile.path()).expect("Could not get absolute path of source");

        // Get the absolute filepath to a temporary destination file
        let destfile = NamedTempFile::new().expect("Could not open a temporary destination file");
        let destination =
            absolute(destfile.path()).expect("Could not get absolute path of destination file");

        // Create the file link
        let link = FileLink {
            source,
            destination,
        };

        // Return the file link and filepaths
        (link, srcfile, destfile)
    }

    /// Creates a new file link for tests, with a source file and destination directory created
    fn create_new_unwritten_filelink() -> (FileLink, NamedTempFile, TempDir) {
        // Get the absolute filepath to a temporary source file
        let srcfile = NamedTempFile::new().expect("Could not create temporary source file");
        let source = absolute(srcfile.path()).expect("Could not get absolute path of source");

        // Get the absolute filepath to a temporary destination directory
        let destdir = tempdir().expect("Could not open a temporary destination file");
        let destination = absolute(destdir.path())
            .expect("Could not get absolute path of destination file")
            .join("testfile");

        // Create the file link
        let link = FileLink {
            source,
            destination,
        };

        // Return the file link and filepaths
        (link, srcfile, destdir)
    }

    mod filelink {

        use super::*;

        mod new {

            use super::*;

            use std::env::current_dir;

            /// Tests FileLink::new(), where:
            ///
            /// - Source file that does exist (absolute path)
            /// - Destination file (absolute path)
            #[test]
            fn success() {
                // Get the absolute filepath to a temporary source file
                let srcfile = NamedTempFile::new().expect("Could not create temporary source file");
                let source =
                    absolute(srcfile.path()).expect("Could not get absolute path of source");

                // Get the absolute filepath to a temporary destination file
                let destfile =
                    NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path())
                    .expect("Could not get absolute path of destination file");

                // Test creating the file link
                let _: FileLink =
                    FileLink::new(&source, &destination).expect("Could not create a valid link");
            }

            /// Tests FileLink::new(), where:
            ///
            /// - Source file that does not exist (absolute path)
            /// - Destination file (absolute path)
            #[test]
            fn source_does_not_exist() {
                // Get the absolute filepath to a nonexistent file
                let source =
                    absolute("does/not/exist").expect("Could not get absolute path of source");

                // Get the absolute filepath to a temporary destination file
                let destfile =
                    NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path())
                    .expect("Could not get absolute path of destination file");

                // Check that an error is returned when creating a file link
                let error = FileLink::new(&source, &destination).expect_err(
                    "Successfully created the file link when it should have been prevented",
                );
                assert_eq!(error, FileLinkCreationError::InvalidSource);
            }

            /// Tests FileLink::new(), where:
            ///
            /// - Source directory that does exist (absolute path)
            /// - Destination file (absolute path)
            #[test]
            fn source_directory() {
                // Get the absolute filepath to a temporary directory
                let srcfile = tempdir().expect("Could not open a temporary source directory");
                let source =
                    absolute(srcfile.path()).expect("Could not get absolute path of source");

                // Get the absolute filepath to a temporary destination file
                let destfile =
                    NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path())
                    .expect("Could not get absolute path of destination file");

                // Check that an error is returned when creating a file link
                let error = FileLink::new(&source, &destination).expect_err(
                    "Successfully created the file link when it should have been prevented",
                );
                assert_eq!(error, FileLinkCreationError::InvalidSource);
            }

            /// Tests FileLink::new(), where:
            ///
            /// - Source file that does exist (relative path)
            /// - Destination file (absolute path)
            #[test]
            fn source_relative() {
                // Get the filepath to the current directory
                let current_dir = current_dir().expect("Could not get current directory");

                // Create a temporary file to use as the source
                let src = PathBuf::from("tests/assets/tempsrcfile");
                let srcfile = absolute(src).expect("Could not get source as absolute path");
                fs::File::create(&srcfile).expect("Could not create temporary file");

                // Get the relative filepath to a temporary source file
                let source = pathdiff::diff_paths(&srcfile, &current_dir)
                    .expect("Could not get relative path for source file");
                assert!(!source.is_absolute());

                // Get the absolute filepath to a temporary destination file
                let destfile =
                    NamedTempFile::new().expect("Could not open a temporary destination file");
                let destination = absolute(destfile.path())
                    .expect("Could not get absolute path of destination file");

                // Check that an error is returned when creating a file link
                let error = FileLink::new(&source, &destination).expect_err(
                    "Successfully created the file link when it should have been prevented",
                );

                // Remove the temporary source file
                fs::remove_file(&source).expect("Could not remove temporary source file");

                // Check that the correct error is returned
                assert_eq!(error, FileLinkCreationError::InvalidSource);
            }

            /// Tests FileLink::new(), where
            ///
            /// = Source file that does exist (absolute path)
            /// - Destination file (relative path)
            #[test]
            fn destination_relative() {
                // Get the filepath to the current directory
                let current_dir = current_dir().expect("Could not get current directory");

                // Get the absolute filepath to a temporary source file
                let srcfile = NamedTempFile::new().expect("Could not create temporary file");
                let source =
                    absolute(srcfile.path()).expect("Could not get absolute path of source");

                // Create a temporary file to use as the destination
                let dest = PathBuf::from("tests/assets/tempdestfile");
                let destfile = absolute(dest).expect("Could not get destination as absolute path");
                fs::File::create(&destfile).expect("Could not create temporary file");

                // Get the relative filepath to a temporary destination file
                let destination = pathdiff::diff_paths(destfile, &current_dir)
                    .expect("Could not get relative path for destination file");
                assert!(!destination.is_absolute());

                // Check that an error is returned when creating a file link
                let error = FileLink::new(&source, &destination).expect_err(
                    "Successfully created the file link when it should have been prevented",
                );

                // Remove the temporary destination file
                fs::remove_file(&source).expect("Could not remove temporary source file");

                // Check that the correct error is returned
                assert_eq!(error, FileLinkCreationError::InvalidDestination);
            }
        }

        mod ensure_writepath {

            use super::*;

            /// Tests FileLink::ensure_writepath(), where it:
            ///
            /// - Successfully creates destination file
            #[test]
            fn destination_does_not_exist() {
                // Generate a file link
                let (mut filelink, _src, _dst) = create_new_unwritten_filelink();

                // Set the destination to a nonexistent file and check it doesn't already exist
                filelink.destination = filelink.destination.join("inner").join("newfile");
                assert!(!filelink.destination.as_path().parent().unwrap().exists());

                // Ensure the write path
                filelink
                    .ensure_writepath()
                    .expect("Could not ensure file link destination");

                // Check the write path now exists
                assert!(filelink.destination.as_path().parent().unwrap().is_dir());
            }

            /// Tests FileLink::ensure_writepath(), where it:
            ///
            /// - Does nothing as the file already exists
            #[test]
            fn destination_exists() {
                // Generate a file link
                let (filelink, _src, _dst) = create_new_filelink();

                // Check the destination already exists
                assert!(filelink.destination.as_path().is_file());

                // Ensure the write path
                filelink
                    .ensure_writepath()
                    .expect("Could not ensure file link destination");

                // Check the write path still exists
                assert!(filelink.destination.as_path().is_file());
            }

            /// Tests FileLink::ensure_writepath(), where:
            ///
            /// - Fail to ensure write path because recursively creatings directories fails
            #[test]
            fn directory_creation_failure() {
                // Generate a file link
                let (mut filelink, _src, _dst) = create_new_filelink();

                // Set the source to a file whose parent is an existing file
                filelink.destination = filelink.source.join("testfile");
                assert!(filelink.destination.parent().unwrap().is_file());

                // Check ensuring the write ptah causes an error
                let error = filelink
                    .ensure_writepath()
                    .expect_err("Successfully ensured an impossible destination");
                assert_eq!(error, FileLinkCreationError::DestinationSetup);

                // Check the state of the destination file
                assert!(filelink.destination.parent().unwrap().is_file());
                assert!(!filelink.destination.parent().unwrap().is_dir());
            }
        }

        mod is_outdated {

            use super::*;

            /// Tests FileLink::is_outdated(), where:
            ///
            /// - Destination file is outdated
            #[test]
            fn source_before_destination() {
                // Generate a file link
                let (link, _src, _dst) = create_new_filelink();

                // Set the destination modification time to 30 seconds before the source
                let orig_mtime = get_file_mtime(&link.source);
                let new_mtime = FileTime::from_unix_time(
                    orig_mtime.unix_seconds() - 30,
                    orig_mtime.nanoseconds(),
                );
                set_file_mtime(&link.destination, new_mtime)
                    .expect("Could not set modification time");

                // Check the file link is identified as outdated
                assert!(link.is_outdated());
            }

            /// Tests FileLink::is_outdated(), where:
            ///
            /// - Destination file is not outdated (it is equal to source)
            #[test]
            fn source_equals_destination() {
                // Generate a file link
                let (link, _src, _dst) = create_new_filelink();

                // Set the destination modification time to the same time as the source
                let orig_mtime = get_file_mtime(&link.source);
                set_file_mtime(&link.destination, orig_mtime)
                    .expect("Could not set modification time");

                // Check the file link is identified as not outdated
                assert!(!link.is_outdated());
            }

            /// Tests FileLink::is_outdated(), where:
            ///
            /// - Destination file is not outdated (it is after source)
            #[test]
            fn source_after_destination() {
                // Generate a file link
                let (link, _src, _dst) = create_new_filelink();

                // Set the destination modification time to 30 seconds after the source
                let orig_mtime = get_file_mtime(&link.source);
                let new_mtime = FileTime::from_unix_time(
                    orig_mtime.unix_seconds() + 30,
                    orig_mtime.nanoseconds(),
                );
                set_file_mtime(&link.destination, new_mtime)
                    .expect("Could not set modification time");

                // Check the file link is identified as not outdated
                assert!(!link.is_outdated());
            }

            /// Tests FileLink::is_outdated(), where:
            ///
            /// - Destination file does not exist
            #[test]
            fn desination_does_not_exist() {
                let (link, _src, _dst) = create_new_unwritten_filelink();
                assert!((link.is_outdated()))
            }
        }

        mod update {

            use std::io::Write;

            use super::*;

            /// Tests the successful use case of FileLink::update()
            #[test]
            fn success() {
                // Generate a file link
                let (mut link, mut src, _dst) = create_new_filelink();

                // Write to the source file
                let new_contents = b"test";
                src.write(new_contents)
                    .expect("Could not write to source file");

                // Update the file link
                let total: u64 = link.update().expect("Could not update file link");

                // Get the contents of the source and destination files
                let src_contents = fs::read(&link.source).expect("Could not read source");
                let dst_contents = fs::read(&link.destination).expect("Could not read destination");

                // Check the contents match
                assert_eq!(src_contents, dst_contents);

                // Check the reported number of bytes copied match
                assert_eq!(total, new_contents.len() as u64);
            }

            /// Tests the use case where FileLink::update() would fail
            #[test]
            fn copy_failed() {
                // Generate a file link
                let (mut link, mut _src, _dst) = create_new_filelink();

                // Set the source to a nonexistent file
                link.source = link.source.join("does/not/exist");

                // Check that update the file link returns an error
                let error = link
                    .update()
                    .expect_err("Updated using nonexistent source file");
                assert_eq!(error, FileUpdateError::CopyFailed);
            }
        }

        /// Tests FileLink::delete()
        #[test]
        fn delete() {
            // Generate a file link
            let (link, _src, _dst) = create_new_unwritten_filelink();

            // Check that the file link destination does not yet exist
            assert!(!link.destination.as_path().exists());

            // Create a file at the file link destination
            fs::File::create(&link.destination).expect("Could not create new destination file");
            assert!(link.destination.as_path().is_file());

            // Perform a file link deletion
            link.delete()
                .expect("Could not delete file link destination file");

            // Check that the file link's destination file no longer exists
            assert!(!link.destination.as_path().exists());
        }

        mod trait_tabled {

            use std::iter::zip;

            use super::*;

            /// Tests the implementation of FileLink::fields() for the Tabled trait
            #[test]
            fn fields() {
                // Generate a file link
                let (link, src, dst) = create_new_filelink();

                // Get the Tabled fields for the file link
                let fields = link.fields();

                // Caclulate what the intended fileds for the file link should be
                let source_path = src
                    .path()
                    .to_str()
                    .expect("Could not get source path as string");
                let destination_path = dst
                    .path()
                    .to_str()
                    .expect("Could not get destination path as string");
                let intendeds = vec![source_path, destination_path];

                // Check accuracy for each field
                for (field, intended) in zip(fields, intendeds) {
                    assert_eq!(field, intended);
                }
            }

            /// Tests the implementation of FileLink::headers() for the Tabled trait
            #[test]
            fn headers() {
                // Get the Tabled headers for the file link
                let headers = FileLink::headers();

                // Store what the intended headers for the file link should be
                let intendeds = vec!["Source", "Destination"];

                // Check accuracy for each header
                for (header, intended) in zip(headers, intendeds) {
                    assert_eq!(header, intended);
                }
            }
        }
    }
}
