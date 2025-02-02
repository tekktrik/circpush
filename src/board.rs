// SPDX-FileCopyrightText: 2025 Alec Delaney
// SPDX-License-Identifier: MIT

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

    /// Tests the ability to detect a connected CircuitPython board
    #[test]
    #[serial_test::serial]
    fn detection() {
        // Find the connected CircuitPython board
        let mount_point: PathBuf = find_circuitpy().expect("Could not find CircuitPython board");

        // Get the filename and filepath of the boot_out.txt file
        let filename = "boot_out.txt";
        let bootout_filepath = mount_point.as_path().join(&filename);

        // Get the filepath of the boot_out.txt test asset file
        let current_filepath = PathBuf::from(file!());
        let parent_filepath = current_filepath.parent().unwrap();
        let grandparent_filepath = parent_filepath.parent().unwrap();
        let asset_filepath = grandparent_filepath
            .join("tests")
            .join("assets")
            .join(&filename);

        // Get the contents of the boot_out.txt test asset file
        let boutout_contents =
            fs::read_to_string(&asset_filepath).expect("Could not read test asset of boot_out.txt");

        // Delete the boot_out.txt on the connected mount
        fs::remove_file(&bootout_filepath).expect("Could not delete file");
        assert!(!bootout_filepath.as_path().exists());

        // Assert that the board is no longer detected
        assert!(find_circuitpy().is_none());

        // Return the boot_out.txt file to the connected mount
        fs::write(&bootout_filepath, &boutout_contents)
            .expect("Could not copy test bootout file after test");
        assert!(bootout_filepath.as_path().is_file());
    }
}
