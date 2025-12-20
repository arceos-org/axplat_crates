use axplat::mem::{MemIf, PhysAddr, RawRange, VirtAddr, pa, va};

use crate::config::devices::MMIO_RANGES;
use crate::config::plat::{
    KERNEL_BASE_PADDR, PHYS_MEMORY_BASE, PHYS_MEMORY_SIZE, PHYS_VIRT_OFFSET,
};

struct MemIfImpl;

#[impl_plat_interface]
impl MemIf for MemIfImpl {
    /// Returns all physical memory (RAM) ranges on the platform.
    ///
    /// All memory ranges except reserved ranges (including the kernel loaded
    /// range) are free for allocation.
    fn phys_ram_ranges() -> &'static [RawRange] {
        // Try to get memory ranges from device tree first
        if let Some(dtb_parser) = crate::dtb::get() {
            let dtb_ranges = dtb_parser.get_memory_ranges();
            if !dtb_ranges.is_empty() {
                // Convert DTB ranges to static format
                // We use a simple approach with a fixed-size array for now
                static mut DTB_RANGES: [RawRange; 4] = [(0, 0); 4];
                static mut RANGE_COUNT: usize = 0;
                static mut INITIALIZED: bool = false;
                
                unsafe {
                    if !INITIALIZED {
                        RANGE_COUNT = dtb_ranges.len().min(4);
                        for (i, range) in dtb_ranges.iter().take(4).enumerate() {
                            DTB_RANGES[i] = (range.base as usize, range.size as usize);
                        }
                        INITIALIZED = true;
                    }
                    
                    if RANGE_COUNT > 0 {
                        return &DTB_RANGES[..RANGE_COUNT];
                    }
                }
            }
        }
        
        // Fallback to configuration-based ranges
        static DEFAULT_RANGES: [RawRange; 1] = [(
            KERNEL_BASE_PADDR,
            PHYS_MEMORY_BASE + PHYS_MEMORY_SIZE - KERNEL_BASE_PADDR,
        )];
        &DEFAULT_RANGES
    }

    /// Returns all reserved physical memory ranges on the platform.
    ///
    /// Reserved memory can be contained in [`phys_ram_ranges`], they are not
    /// allocatable but should be mapped to kernel's address space.
    ///
    /// Note that the ranges returned should not include the range where the
    /// kernel is loaded.
    fn reserved_phys_ram_ranges() -> &'static [RawRange] {
        &[]
    }

    /// Returns all device memory (MMIO) ranges on the platform.
    fn mmio_ranges() -> &'static [RawRange] {
        &MMIO_RANGES
    }

    /// Translates a physical address to a virtual address.
    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        va!(paddr.as_usize() + PHYS_VIRT_OFFSET)
    }

    /// Translates a virtual address to a physical address.
    fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
        pa!(vaddr.as_usize() - PHYS_VIRT_OFFSET)
    }
}
