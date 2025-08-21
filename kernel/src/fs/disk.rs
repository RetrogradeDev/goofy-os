use crate::fs::fat32::DiskOperations;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

/// Primary ATA controller ports
const PRIMARY_ATA_DATA: u16 = 0x1F0;
// const PRIMARY_ATA_ERROR: u16 = 0x1F1;
// const PRIMARY_ATA_FEATURES: u16 = 0x1F1;
const PRIMARY_ATA_SECTOR_COUNT: u16 = 0x1F2;
const PRIMARY_ATA_LBA_LOW: u16 = 0x1F3;
const PRIMARY_ATA_LBA_MID: u16 = 0x1F4;
const PRIMARY_ATA_LBA_HIGH: u16 = 0x1F5;
const PRIMARY_ATA_DRIVE: u16 = 0x1F6;
const PRIMARY_ATA_STATUS: u16 = 0x1F7;
const PRIMARY_ATA_COMMAND: u16 = 0x1F7;
const PRIMARY_ATA_CONTROL: u16 = 0x3F6;

/// ATA commands
const ATA_CMD_READ_SECTORS: u8 = 0x20;
const ATA_CMD_WRITE_SECTORS: u8 = 0x30;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

/// ATA status bits
const ATA_STATUS_BSY: u8 = 0x80;
const ATA_STATUS_DRDY: u8 = 0x40;
// const ATA_STATUS_DF: u8 = 0x20;
const ATA_STATUS_DRQ: u8 = 0x08;
const ATA_STATUS_ERR: u8 = 0x01;

/// Simple ATA disk driver
pub struct AtaDisk {
    data_port: Port<u16>,
    // error_port: PortReadOnly<u8>,
    // features_port: PortWriteOnly<u8>,
    sector_count_port: Port<u8>,
    lba_low_port: Port<u8>,
    lba_mid_port: Port<u8>,
    lba_high_port: Port<u8>,
    drive_port: Port<u8>,
    status_port: PortReadOnly<u8>,
    command_port: PortWriteOnly<u8>,
    control_port: PortWriteOnly<u8>,
    drive_number: u8,
}

impl AtaDisk {
    /// Create a new ATA disk driver for the primary controller
    pub fn new_primary(drive_number: u8) -> Self {
        AtaDisk {
            data_port: Port::new(PRIMARY_ATA_DATA),
            // error_port: PortReadOnly::new(PRIMARY_ATA_ERROR),
            // features_port: PortWriteOnly::new(PRIMARY_ATA_FEATURES),
            sector_count_port: Port::new(PRIMARY_ATA_SECTOR_COUNT),
            lba_low_port: Port::new(PRIMARY_ATA_LBA_LOW),
            lba_mid_port: Port::new(PRIMARY_ATA_LBA_MID),
            lba_high_port: Port::new(PRIMARY_ATA_LBA_HIGH),
            drive_port: Port::new(PRIMARY_ATA_DRIVE),
            status_port: PortReadOnly::new(PRIMARY_ATA_STATUS),
            command_port: PortWriteOnly::new(PRIMARY_ATA_COMMAND),
            control_port: PortWriteOnly::new(PRIMARY_ATA_CONTROL),
            drive_number: drive_number & 1, // Ensure it's 0 or 1
        }
    }

    /// Wait for the drive to be ready
    fn wait_ready(&mut self) -> Result<(), &'static str> {
        let mut timeout = 10000;
        while timeout > 0 {
            let status = unsafe { self.status_port.read() };
            if (status & ATA_STATUS_BSY) == 0 && (status & ATA_STATUS_DRDY) != 0 {
                return Ok(());
            }
            timeout -= 1;
        }
        Err("ATA drive timeout waiting for ready")
    }

    /// Wait for data to be ready
    fn wait_data(&mut self) -> Result<(), &'static str> {
        let mut timeout = 10000;
        while timeout > 0 {
            let status = unsafe { self.status_port.read() };
            if (status & ATA_STATUS_BSY) == 0 {
                if (status & ATA_STATUS_DRQ) != 0 {
                    return Ok(());
                }
                if (status & ATA_STATUS_ERR) != 0 {
                    return Err("ATA drive error");
                }
            }
            timeout -= 1;
        }
        Err("ATA drive timeout waiting for data")
    }

    /// Select the drive
    fn select_drive(&mut self, lba: u64) -> Result<(), &'static str> {
        if lba >= (1u64 << 28) {
            return Err("LBA too large for 28-bit LBA");
        }

        // Select drive and set LBA mode with upper 4 bits of LBA
        let drive_byte = 0xE0 | (self.drive_number << 4) | ((lba >> 24) & 0x0F) as u8;
        unsafe {
            self.drive_port.write(drive_byte);
        }

        // Small delay after drive selection
        for _ in 0..4 {
            unsafe {
                self.status_port.read();
            }
        }

        Ok(())
    }

    /// Initialize the disk
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Disable interrupts on the ATA controller to avoid conflicts
        unsafe {
            self.control_port.write(0x02); // Set nIEN bit to disable interrupts
        }

        // Reset the controller
        unsafe {
            self.control_port.write(0x06); // Set reset bit + nIEN
            for _ in 0..1000 {} // Small delay
            self.control_port.write(0x02); // Clear reset bit but keep nIEN
        }

        // Wait for drive to be ready
        self.wait_ready()?;

        // Try to identify the drive
        self.select_drive(0)?;
        unsafe {
            self.sector_count_port.write(0);
            self.lba_low_port.write(0);
            self.lba_mid_port.write(0);
            self.lba_high_port.write(0);
            self.command_port.write(ATA_CMD_IDENTIFY);
        }

        // Check if drive exists
        let status = unsafe { self.status_port.read() };
        if status == 0 {
            return Err("Drive does not exist");
        }

        // Wait for data
        match self.wait_data() {
            Ok(_) => {
                // Read and discard identify data
                for _ in 0..256 {
                    unsafe {
                        self.data_port.read();
                    }
                }
                Ok(())
            }
            Err(_) => {
                // Drive exists but may not support IDENTIFY (some virtual drives)
                // This is okay, we can still try to use it
                Ok(())
            }
        }
    }
}

impl DiskOperations for AtaDisk {
    fn read_sector(&mut self, sector: u64, buffer: &mut [u8]) -> Result<(), &'static str> {
        if buffer.len() != 512 {
            return Err("Buffer must be exactly 512 bytes");
        }

        self.wait_ready()?;
        self.select_drive(sector)?;

        // Set up the read operation
        unsafe {
            self.sector_count_port.write(1); // Read 1 sector
            self.lba_low_port.write((sector & 0xFF) as u8);
            self.lba_mid_port.write(((sector >> 8) & 0xFF) as u8);
            self.lba_high_port.write(((sector >> 16) & 0xFF) as u8);
            self.command_port.write(ATA_CMD_READ_SECTORS);
        }

        // Wait for data to be ready
        self.wait_data()?;

        // Read the data
        let words =
            unsafe { core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u16, 256) };

        for word in words.iter_mut() {
            *word = unsafe { self.data_port.read() };
        }

        Ok(())
    }

    fn write_sector(&mut self, sector: u64, buffer: &[u8]) -> Result<(), &'static str> {
        if buffer.len() != 512 {
            return Err("Buffer must be exactly 512 bytes");
        }

        self.wait_ready()?;
        self.select_drive(sector)?;

        // Set up the write operation
        unsafe {
            self.sector_count_port.write(1); // Write 1 sector
            self.lba_low_port.write((sector & 0xFF) as u8);
            self.lba_mid_port.write(((sector >> 8) & 0xFF) as u8);
            self.lba_high_port.write(((sector >> 16) & 0xFF) as u8);
            self.command_port.write(ATA_CMD_WRITE_SECTORS);
        }

        // Wait for drive to be ready for data
        self.wait_data()?;

        // Write the data
        let words = unsafe { core::slice::from_raw_parts(buffer.as_ptr() as *const u16, 256) };

        for &word in words.iter() {
            unsafe {
                self.data_port.write(word);
            }
        }

        // Wait for write to complete
        self.wait_ready()?;

        Ok(())
    }
}
