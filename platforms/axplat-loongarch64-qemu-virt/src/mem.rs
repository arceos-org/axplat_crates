use axplat::mem::{MemIf, PhysAddr, RawRange, VirtAddr, pa, va};

use crate::config::devices::MMIO_RANGES;
use crate::config::plat::{PHYS_MEMORY_BASE, PHYS_MEMORY_SIZE, PHYS_VIRT_OFFSET};

/// DMW0 offset for uncached device memory access.
/// LoongArch64 DMW0 is configured as: 0x8000_xxxx_xxxx_xxxx -> physical address (uncached)
const DMW0_OFFSET: usize = 0x8000_0000_0000_0000;

struct MemIfImpl;

#[allow(dead_code)]
pub const fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    va!(paddr.as_usize() + PHYS_VIRT_OFFSET)
}

/// Converts a physical address to a virtual address for MMIO device access.
/// Uses DMW0 (uncached) mapping which is required for device memory.
#[allow(dead_code)]
pub const fn mmio_phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    va!(paddr.as_usize() + DMW0_OFFSET)
}

#[impl_plat_interface]
impl MemIf for MemIfImpl {
    /// Returns all physical memory (RAM) ranges on the platform.
    ///
    /// All memory ranges except reserved ranges (including the kernel loaded
    /// range) are free for allocation.
    fn phys_ram_ranges() -> &'static [RawRange] {
        &[(PHYS_MEMORY_BASE, PHYS_MEMORY_SIZE)]
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
        phys_to_virt(paddr)
    }

    /// Translates a virtual address to a physical address.
    fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
        pa!(vaddr.as_usize() - PHYS_VIRT_OFFSET)
    }
}
