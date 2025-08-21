use crate::fs::disk::AtaDisk;
use crate::fs::fat32::{Fat32FileSystem, FileEntry};
use alloc::string::String;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref FILESYSTEM: Mutex<Option<Fat32FileSystem<AtaDisk>>> = Mutex::new(None);
}

/// Initialize the filesystem
pub fn init_filesystem() -> Result<(), &'static str> {
    crate::serial_println!("Initializing filesystem...");

    // Try primary master first (drive 0)
    crate::serial_println!("Trying primary master drive (0)...");
    let mut disk = AtaDisk::new_primary(0);
    if let Ok(_) = disk.init() {
        crate::serial_println!("Primary master initialized successfully");
        match Fat32FileSystem::new(disk) {
            Ok(filesystem) => {
                crate::serial_println!("FAT32 filesystem found on primary master");
                *FILESYSTEM.lock() = Some(filesystem);
                return Ok(());
            }
            Err(e) => {
                crate::serial_println!("Primary master is not FAT32: {}", e);
            }
        }
    } else {
        crate::serial_println!("Failed to initialize primary master");
    }

    // Try primary slave (drive 1)
    crate::serial_println!("Trying primary slave drive (1)...");
    let mut disk = AtaDisk::new_primary(1);
    if let Ok(_) = disk.init() {
        crate::serial_println!("Primary slave initialized successfully");
        match Fat32FileSystem::new(disk) {
            Ok(filesystem) => {
                crate::serial_println!("FAT32 filesystem found on primary slave");
                *FILESYSTEM.lock() = Some(filesystem);
                return Ok(());
            }
            Err(e) => {
                crate::serial_println!("Primary slave is not FAT32: {}", e);
            }
        }
    } else {
        crate::serial_println!("Failed to initialize primary slave");
    }

    Err("No FAT32 filesystem found on any drive")
}

/// List files in the root directory
pub fn list_root_files() -> Result<Vec<FileEntry>, &'static str> {
    let mut fs_guard = FILESYSTEM.lock();
    match fs_guard.as_mut() {
        Some(fs) => fs.list_root_directory(),
        None => Err("Filesystem not initialized"),
    }
}

/// List files in a directory by cluster
pub fn list_directory_files(cluster: u32) -> Result<Vec<FileEntry>, &'static str> {
    let mut fs_guard = FILESYSTEM.lock();
    match fs_guard.as_mut() {
        Some(fs) => fs.list_directory(cluster),
        None => Err("Filesystem not initialized"),
    }
}

/// Find a file in the root directory
pub fn find_file_in_root(filename: &str) -> Result<Option<FileEntry>, &'static str> {
    let mut fs_guard = FILESYSTEM.lock();
    match fs_guard.as_mut() {
        Some(fs) => fs.find_file_in_root(filename),
        None => Err("Filesystem not initialized"),
    }
}

/// Find a file in a specific directory
pub fn find_file_in_directory(
    dir_cluster: u32,
    filename: &str,
) -> Result<Option<FileEntry>, &'static str> {
    let mut fs_guard = FILESYSTEM.lock();
    match fs_guard.as_mut() {
        Some(fs) => fs.find_file_in_directory(dir_cluster, filename),
        None => Err("Filesystem not initialized"),
    }
}

/// Read a file's content
pub fn read_file(first_cluster: u32, file_size: u32) -> Result<Vec<u8>, &'static str> {
    let mut fs_guard = FILESYSTEM.lock();
    match fs_guard.as_mut() {
        Some(fs) => fs.read_file(first_cluster, file_size),
        None => Err("Filesystem not initialized"),
    }
}

/// Read a text file and return it as a string
pub fn read_text_file(first_cluster: u32, file_size: u32) -> Result<String, &'static str> {
    let data = read_file(first_cluster, file_size)?;
    match String::from_utf8(data) {
        Ok(text) => Ok(text),
        Err(_) => Err("File is not valid UTF-8"),
    }
}
