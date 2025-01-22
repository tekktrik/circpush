use std::path::PathBuf;

use sysinfo::Disks;

/// Find the connected CircuitPython board.
///
/// On success, returns the path of the board as a PathBuf.
/// On error, return None.
pub fn find_circuitpy() -> Option<PathBuf> {
    for disk in Disks::new_with_refreshed_list().list() {
        let mount_point = disk.mount_point();
        if mount_point.join("boot_out.txt").is_file() {
            return Some(mount_point.to_path_buf());
        }
    }
    None
}

#[cfg(test)]
mod test {

    use std::fs;

    use super::*;

    #[test]
    #[serial_test::serial]
    fn states() {
        let mount_point: PathBuf = find_circuitpy().expect("Could not find CircuitPython board");

        let filename = "boot_out.txt";

        let bootout_filepath = mount_point.join(&filename);

        let current_filepath = PathBuf::from(file!());
        let parent_filepath = current_filepath.parent().unwrap();
        let grandparent_filepath = parent_filepath.parent().unwrap();
        let asset_filepath = grandparent_filepath.join("tests").join("assets").join(&filename);
        let boutout_contents = fs::read_to_string(&asset_filepath).expect("Could not read test asset of boot_out.txt");

        fs::remove_file(&bootout_filepath).expect("Could not delete file");
        assert!(!bootout_filepath.exists());

        assert!(find_circuitpy().is_none());

        fs::write(&bootout_filepath, &boutout_contents).expect("Could not copy test bootout file after test");
        assert!(bootout_filepath.exists());
    }
}
