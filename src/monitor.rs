use std::{collections::HashSet, path::{absolute, Path, PathBuf}};
use crate::link::FileLink;
use glob::{glob, Paths};
use pathdiff::diff_paths;
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};


#[derive(Debug)]
pub enum UpdateError {
    PartialGlobMatch,
    FileIOError,
}

#[derive(Debug)]
pub enum PathError {
    NoAbsolute,
    NoRelative,
}

#[derive(Serialize, Deserialize)]
pub struct FileMonitor {
    read_pattern: String,
    write_directory: PathBuf,
    base_directory: PathBuf,
    links: HashSet<FileLink>,
}

// impl Tabled for FileMonitor {

//     const LENGTH: usize = 3;

//     fn fields(&self) -> Vec<std::borrow::Cow<'_, str>> {
//         todo!()
//     }

//     fn headers() -> Vec<std::borrow::Cow<'static, str>> {
//         Table::builder(iter).index()
//     }
// }

impl FileMonitor {

    pub fn new(read_pattern: String, write_directory: PathBuf, base_directory: PathBuf) -> Result<Self, PathError> {
        let base_directory = match absolute(Path::new(&base_directory)) {
            Ok(abspath) => {abspath.to_path_buf()},
            Err(_) => return Err(PathError::NoAbsolute),
        };

        let write_directory = match absolute(base_directory.join(&write_directory)) {
            Ok(abspath) => {abspath.to_path_buf()},
            Err(_) => return Err(PathError::NoAbsolute),
        };

        let file_monitor = Self {
            read_pattern,
            write_directory,
            base_directory,
            links: HashSet::new(),
        };

        Ok(file_monitor)
    }

    fn get_write_path(&self, filepath: &PathBuf) -> Result<PathBuf, PathError> {
        match diff_paths(filepath, &self.base_directory) {
            Some(relative_path) => {
                let joinpath = self.write_directory.join(relative_path);
                Ok(absolute(joinpath).expect("Could not create absolute write path"))
            },
            None => Err(PathError::NoRelative),
        } 
    }

    fn iterate_paths(&self, paths: Paths) -> Result<HashSet<FileLink>, UpdateError> {
        let mut new_hashset = HashSet::new();
        for read_path in paths.map(|result| result.expect("Could not read all glob matches")).filter(|path| path.is_file()) {
            let abs_read_path = absolute(&read_path).expect("Unable to create absolute path");
            let abs_write_path = self.get_write_path(&read_path).expect("Could not get write path wile iterating paths");
            let filelink = FileLink::new(&abs_read_path, &abs_write_path).expect("Could not create new FileLink");
            new_hashset.insert(filelink);
        }
        Ok(new_hashset)
    }

    pub fn calculate_monitored_files(&self) -> Result<HashSet<FileLink>, UpdateError> {
        let abs_read_directory = self.base_directory.join(&self.read_pattern);
        let read_dir_str = abs_read_directory.to_str().expect("Invalid read directory");
        match glob(read_dir_str) {
            Ok(paths) => Ok(self.iterate_paths(paths)?),
            Err(_) => { Err(UpdateError::PartialGlobMatch) },
        }
    }

    pub fn update_links(&mut self) -> Result<(), UpdateError> {
        let new_filelinks = self.calculate_monitored_files()?;

        for removed_file in self.links.difference(&new_filelinks) {
            if removed_file.delete().is_err() { return Err(UpdateError::FileIOError) }
        }

        let mut new_filelinks_vec = Vec::from_iter(new_filelinks);

        for new_filelink in &mut new_filelinks_vec {
            if new_filelink.is_outdated() {
                new_filelink.ensure_writepath().expect("Could not ensure write path");
                new_filelink.update().expect("Could not update linked files");
            }
        }

        let new_filelinks = HashSet::from_iter(new_filelinks_vec);

        self.links = new_filelinks;
        Ok(())

    }

    pub fn serialize(&self) -> String {
        serde_json::to_string(self).expect("Could not serialize the FileMonitor")
    }

    pub fn deserialize(text: &str) -> Self {
        serde_json::from_str(text).expect("Could not deserialize FileMonitor")
    }

    pub fn to_table_record(&self) -> Vec<String> {
        vec![
            self.read_pattern.to_owned(),
            String::from(self.base_directory.to_str().expect("Could not convert base directory to String")),
            String::from(self.write_directory.to_str().expect("Could not convert write directory to String")),
        ]
    }

    pub fn table_header() -> Vec<&'static str> {
        vec!["Read Pattern", "Base Directory", "Write Directory"]
    }

}
