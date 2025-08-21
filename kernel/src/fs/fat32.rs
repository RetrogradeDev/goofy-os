use crate::serial_println;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::mem;

/// Boot sector of a FAT32 filesystem
#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct Fat32BootSector {
    pub jump_instruction: [u8; 3],
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_dir_entries: u16,
    pub total_sectors_16: u16,
    pub media_descriptor: u8,
    pub sectors_per_fat_16: u16,
    pub sectors_per_track: u16,
    pub head_count: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,

    // FAT32 specific fields
    pub sectors_per_fat_32: u32,
    pub ext_flags: u16,
    pub filesystem_version: u16,
    pub root_cluster: u32,
    pub filesystem_info: u16,
    pub backup_boot_sector: u16,
    pub reserved: [u8; 12],
    pub drive_number: u8,
    pub reserved1: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub filesystem_type: [u8; 8],
    pub boot_code: [u8; 420],
    pub bootable_partition_signature: u16,
}

/// Directory entry structure for FAT32
#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirectoryEntry {
    pub name: [u8; 11],
    pub attributes: u8,
    pub reserved: u8,
    pub creation_time_tenths: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub last_access_date: u16,
    pub first_cluster_high: u16,
    pub last_write_time: u16,
    pub last_write_date: u16,
    pub first_cluster_low: u16,
    pub file_size: u32,
}

/// File attributes
pub mod attributes {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    pub const LONG_NAME: u8 = READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID;
}

/// FAT32 cluster values
pub mod cluster_values {
    pub const FREE: u32 = 0x00000000;
    pub const BAD: u32 = 0x0FFFFFF7;
    pub const END_OF_CHAIN: u32 = 0x0FFFFFFF;
    pub const MASK: u32 = 0x0FFFFFFF;
}

/// Represents a file or directory in the FAT32 filesystem
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u32,
    pub first_cluster: u32,
}

/// Trait for disk operations
pub trait DiskOperations {
    fn read_sector(&mut self, sector: u64, buffer: &mut [u8]) -> Result<(), &'static str>;
    fn write_sector(&mut self, sector: u64, buffer: &[u8]) -> Result<(), &'static str>;
}

/// FAT32 filesystem implementation
pub struct Fat32FileSystem<D: DiskOperations> {
    disk: D,
    boot_sector: Fat32BootSector,
    fat_start_sector: u64,
    data_start_sector: u64,
    sectors_per_cluster: u64,
    bytes_per_sector: u64,
}

impl<D: DiskOperations> Fat32FileSystem<D> {
    /// Create a new FAT32 filesystem instance
    pub fn new(mut disk: D) -> Result<Self, &'static str> {
        let mut boot_sector_data = [0u8; 512];
        disk.read_sector(0, &mut boot_sector_data)?;

        let boot_sector = unsafe { *(boot_sector_data.as_ptr() as *const Fat32BootSector) };

        // Copy values to avoid packed struct alignment issues
        let signature = boot_sector.bootable_partition_signature;
        let sectors_per_fat_16 = boot_sector.sectors_per_fat_16;
        let sectors_per_fat_32 = boot_sector.sectors_per_fat_32;
        let root_dir_entries = boot_sector.root_dir_entries;

        // Debug: Print some boot sector information
        serial_println!("Boot sector signature: 0x{:04X}", signature);
        serial_println!("Sectors per FAT (16-bit): {}", sectors_per_fat_16);
        serial_println!("Sectors per FAT (32-bit): {}", sectors_per_fat_32);
        serial_println!("Root dir entries: {}", root_dir_entries);

        // Verify this is a FAT32 filesystem
        if signature != 0xAA55 {
            return Err("Invalid boot sector signature");
        }

        if sectors_per_fat_16 != 0 {
            return Err("This is not a FAT32 filesystem (FAT16/12 detected)");
        }

        let fat_start_sector = boot_sector.reserved_sectors as u64;
        let fat_size = boot_sector.sectors_per_fat_32 as u64;
        let data_start_sector = fat_start_sector + (boot_sector.fat_count as u64 * fat_size);

        let bytes_per_sector = boot_sector.bytes_per_sector;
        let sectors_per_cluster = boot_sector.sectors_per_cluster;
        let reserved_sectors = boot_sector.reserved_sectors;
        let fat_count = boot_sector.fat_count;
        let root_cluster = boot_sector.root_cluster;

        serial_println!("FAT32 Filesystem detected:");
        serial_println!("  Bytes per sector: {}", bytes_per_sector);
        serial_println!("  Sectors per cluster: {}", sectors_per_cluster);
        serial_println!("  Reserved sectors: {}", reserved_sectors);
        serial_println!("  FAT count: {}", fat_count);
        serial_println!("  Root cluster: {}", root_cluster);
        serial_println!("  FAT start sector: {}", fat_start_sector);
        serial_println!("  Data start sector: {}", data_start_sector);

        Ok(Fat32FileSystem {
            disk,
            boot_sector,
            fat_start_sector,
            data_start_sector,
            sectors_per_cluster: boot_sector.sectors_per_cluster as u64,
            bytes_per_sector: boot_sector.bytes_per_sector as u64,
        })
    }

    /// Get the sector number for a given cluster
    fn cluster_to_sector(&self, cluster: u32) -> u64 {
        if cluster < 2 {
            return 0; // Invalid cluster
        }
        self.data_start_sector + (cluster as u64 - 2) * self.sectors_per_cluster
    }

    /// Read a cluster from the disk
    fn read_cluster(&mut self, cluster: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        let sector = self.cluster_to_sector(cluster);
        let cluster_size = self.sectors_per_cluster * self.bytes_per_sector;

        if buffer.len() < cluster_size as usize {
            return Err("Buffer too small for cluster");
        }

        for i in 0..self.sectors_per_cluster {
            let sector_offset = i * self.bytes_per_sector as u64;
            self.disk.read_sector(
                sector + i,
                &mut buffer
                    [sector_offset as usize..(sector_offset + self.bytes_per_sector) as usize],
            )?;
        }

        Ok(())
    }

    /// Read the next cluster from the FAT
    fn get_next_cluster(&mut self, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4; // 4 bytes per FAT32 entry
        let fat_sector = self.fat_start_sector + (fat_offset as u64 / self.bytes_per_sector);
        let sector_offset = (fat_offset as u64 % self.bytes_per_sector) as usize;

        let mut sector_buffer = [0u8; 512];
        self.disk.read_sector(fat_sector, &mut sector_buffer)?;

        let fat_entry = u32::from_le_bytes([
            sector_buffer[sector_offset],
            sector_buffer[sector_offset + 1],
            sector_buffer[sector_offset + 2],
            sector_buffer[sector_offset + 3],
        ]) & cluster_values::MASK;

        Ok(fat_entry)
    }

    /// Read directory entries from a cluster
    fn read_directory_entries(
        &mut self,
        cluster: u32,
    ) -> Result<Vec<DirectoryEntry>, &'static str> {
        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let mut cluster_buffer = vec![0u8; cluster_size];
        let mut entries = Vec::new();
        let mut current_cluster = cluster;

        loop {
            self.read_cluster(current_cluster, &mut cluster_buffer)?;

            let entries_per_cluster = cluster_size / mem::size_of::<DirectoryEntry>();

            for i in 0..entries_per_cluster {
                let entry_offset = i * mem::size_of::<DirectoryEntry>();
                let entry = unsafe {
                    *(cluster_buffer.as_ptr().add(entry_offset) as *const DirectoryEntry)
                };

                // Check if this is the end of directory entries
                if entry.name[0] == 0x00 {
                    return Ok(entries);
                }

                // Skip deleted entries and long filename entries
                if entry.name[0] == 0xE5 || entry.attributes == attributes::LONG_NAME {
                    continue;
                }

                entries.push(entry);
            }

            // Get the next cluster in the chain
            let next_cluster = self.get_next_cluster(current_cluster)?;
            if next_cluster >= cluster_values::END_OF_CHAIN {
                break;
            }
            current_cluster = next_cluster;
        }

        Ok(entries)
    }

    /// Convert a directory entry to a FileEntry
    fn entry_to_file_entry(&self, entry: &DirectoryEntry) -> FileEntry {
        let mut name = String::new();

        // Parse the 8.3 filename format
        let mut i = 0;
        while i < 8 && entry.name[i] != 0x20 {
            name.push(entry.name[i] as char);
            i += 1;
        }

        // Add extension if present
        if entry.name[8] != 0x20 {
            name.push('.');
            let mut i = 8;
            while i < 11 && entry.name[i] != 0x20 {
                name.push(entry.name[i] as char);
                i += 1;
            }
        }

        let first_cluster =
            ((entry.first_cluster_high as u32) << 16) | (entry.first_cluster_low as u32);

        FileEntry {
            name,
            is_directory: (entry.attributes & attributes::DIRECTORY) != 0,
            size: entry.file_size,
            first_cluster,
        }
    }

    /// List files in the root directory
    pub fn list_root_directory(&mut self) -> Result<Vec<FileEntry>, &'static str> {
        let entries = self.read_directory_entries(self.boot_sector.root_cluster)?;
        let mut files = Vec::new();

        for entry in entries {
            // Skip volume labels and system files
            if (entry.attributes & attributes::VOLUME_ID) != 0 {
                continue;
            }

            files.push(self.entry_to_file_entry(&entry));
        }

        Ok(files)
    }

    /// List files in a specific directory
    pub fn list_directory(&mut self, dir_cluster: u32) -> Result<Vec<FileEntry>, &'static str> {
        let entries = self.read_directory_entries(dir_cluster)?;
        let mut files = Vec::new();

        for entry in entries {
            // Skip volume labels
            if (entry.attributes & attributes::VOLUME_ID) != 0 {
                continue;
            }

            files.push(self.entry_to_file_entry(&entry));
        }

        Ok(files)
    }

    /// Read a file's content
    pub fn read_file(
        &mut self,
        first_cluster: u32,
        file_size: u32,
    ) -> Result<Vec<u8>, &'static str> {
        let mut file_data = Vec::new();
        let mut current_cluster = first_cluster;
        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let mut bytes_read = 0u32;

        while bytes_read < file_size {
            let mut cluster_buffer = vec![0u8; cluster_size];
            self.read_cluster(current_cluster, &mut cluster_buffer)?;

            let bytes_to_read =
                core::cmp::min(cluster_size as u32, file_size - bytes_read) as usize;

            file_data.extend_from_slice(&cluster_buffer[..bytes_to_read]);
            bytes_read += bytes_to_read as u32;

            if bytes_read >= file_size {
                break;
            }

            // Get the next cluster
            let next_cluster = self.get_next_cluster(current_cluster)?;
            if next_cluster >= cluster_values::END_OF_CHAIN {
                break;
            }
            current_cluster = next_cluster;
        }

        Ok(file_data)
    }

    /// Find a file in a directory by name
    pub fn find_file_in_directory(
        &mut self,
        dir_cluster: u32,
        filename: &str,
    ) -> Result<Option<FileEntry>, &'static str> {
        let files = self.list_directory(dir_cluster)?;

        for file in files {
            if file.name.to_uppercase() == filename.to_uppercase() {
                return Ok(Some(file));
            }
        }

        Ok(None)
    }

    /// Find a file in the root directory by name
    pub fn find_file_in_root(&mut self, filename: &str) -> Result<Option<FileEntry>, &'static str> {
        self.find_file_in_directory(self.boot_sector.root_cluster, filename)
    }
}
