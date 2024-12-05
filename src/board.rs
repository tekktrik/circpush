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
