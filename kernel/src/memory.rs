use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB,
        mapper::MapToError,
    },
};

// use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use bootloader_api::info::{MemoryRegionKind, MemoryRegions};

use crate::serial_println;

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

/// A FrameAllocator that always returns `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        if frame.is_none() {
            serial_println!("Frame allocation failed at index {}", self.next);
            // Count total available frames for debugging
            let total_frames = self.usable_frames().count();
            serial_println!(
                "Total usable frames: {}, requested index: {}",
                total_frames,
                self.next
            );
        }
        self.next += 1;
        frame
    }
}

pub struct ProcessAddressSpace {
    pub page_table_frame: PhysFrame<Size4KiB>,
    physical_memory_offset: VirtAddr,
}

impl ProcessAddressSpace {
    pub fn new(
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        physical_memory_offset: VirtAddr,
    ) -> Result<Self, MapToError<Size4KiB>> {
        // Create a new page table for the process
        let page_table_frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        // Initialize the page table with kernel mappings
        // but separate user space
        let page_table_virt = physical_memory_offset + page_table_frame.start_address().as_u64();
        let page_table_ptr: *mut PageTable = page_table_virt.as_mut_ptr();

        // Zero out the new page table
        unsafe {
            let page_table = &mut *page_table_ptr;
            page_table.zero();

            // Copy kernel mappings from current page table
            // We need to copy ALL kernel mappings to ensure kernel remains accessible
            let current_table = active_level_4_table(physical_memory_offset);

            // Copy ALL non-empty entries to preserve all kernel mappings
            for i in 0..512 {
                if !current_table[i].is_unused() {
                    page_table[i] = current_table[i].clone();
                }
            }

            // Recursively map the L4 page table to itself for easy access
            page_table[510].set_addr(
                page_table_frame.start_address(),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            );
        }

        Ok(ProcessAddressSpace {
            page_table_frame,
            physical_memory_offset,
        })
    }

    pub fn map_user_memory(
        &mut self,
        virtual_addr: VirtAddr,
        physical_addr: PhysAddr,
        _size: u64, // Currently unused - for future multi-page mappings
        flags: PageTableFlags,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), MapToError<Size4KiB>> {
        use x86_64::structures::paging::Page;

        // Get access to the process's page table
        let page_table_virt =
            self.physical_memory_offset + self.page_table_frame.start_address().as_u64();
        let page_table_ptr: *mut PageTable = page_table_virt.as_mut_ptr();

        unsafe {
            let page_table_ref = &mut *page_table_ptr;
            let mut mapper = OffsetPageTable::new(page_table_ref, self.physical_memory_offset);

            let frame = PhysFrame::containing_address(physical_addr);
            let page = Page::containing_address(virtual_addr);

            serial_println!(
                "Mapping virtual address {:?} to physical address {:?}, page: {:?}, frame: {:?}",
                virtual_addr,
                physical_addr,
                page,
                frame
            );

            // Map memory with user accessible flags
            let final_flags = flags | PageTableFlags::USER_ACCESSIBLE;
            serial_println!("Final page table flags being set: {:?}", final_flags);

            mapper
                .map_to(page, frame, final_flags, frame_allocator)?
                .flush();

            // Check what the page table entry actually contains after mapping
            if let Ok(frame) = mapper.translate_page(page) {
                serial_println!("Page table entry after mapping: frame={:?}", frame);
            }
        }
        Ok(())
    }
}
