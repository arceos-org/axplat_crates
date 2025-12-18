//! Early boot initialization code for ARMv7-A.

use axplat::mem::pa;
use page_table_entry::{GenericPTE, MappingFlags, arm::A32PTE};

use crate::config::plat::BOOT_STACK_SIZE;

/// Boot stack, 256KB
#[unsafe(link_section = ".bss.stack")]
pub static mut BOOT_STACK: [u8; BOOT_STACK_SIZE] = [0; BOOT_STACK_SIZE];

/// ARMv7-A L1 page table (16KB, contains 4096 entries)
/// Must be 16KB aligned for TTBR0
#[repr(align(16384))]
struct Aligned16K<T>(T);

impl<T> Aligned16K<T> {
    const fn new(inner: T) -> Self {
        Self(inner)
    }
}

#[unsafe(link_section = ".data.page_table")]
static mut BOOT_PT_L1: Aligned16K<[A32PTE; 4096]> = Aligned16K::new([A32PTE::empty(); 4096]);

/// Initialize boot page table.
/// This function is unsafe as it modifies global static variables.
#[unsafe(no_mangle)]
pub unsafe fn init_boot_page_table() {
    unsafe {
        // Map memory regions using 1MB sections (ARMv7-A max granularity)
        // Note: AArch64 can use 1GB blocks, but we're limited to 1MB here

        // 0x0000_0000..0xc000_0000 (0-3GB): Normal memory, RWX
        // Equivalent to AArch64's first 3 entries, but needs 3072 entries (3 * 1024)
        for i in 0..0xc00 {
            BOOT_PT_L1.0[i] = A32PTE::new_page(
                pa!(i * 0x10_0000),
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
                true, // 1MB section
            );
        }

        // 0xc000_0000..0x1_0000_0000 (3GB-4GB): Device memory
        // Equivalent to AArch64's BOOT_PT_L1[3], but needs 1024 entries
        // This includes MMIO devices (PL011 UART, GICv2, VirtIO, etc.)
        for i in 0xc00..0x1000 {
            BOOT_PT_L1.0[i] = A32PTE::new_page(
                pa!(i * 0x10_0000),
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
                true,
            );
        }
    }
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "
        // Set stack pointer to top of BOOT_STACK
        ldr sp, ={boot_stack}
        add sp, sp, #{stack_size}
        
        // Initialize page table
        bl {init_pt}
        
        // Enable MMU
        // bl {enable_mmu}
        
        // Jump to Rust entry
        bl {rust_entry}
    1:  b 1b",
        boot_stack = sym BOOT_STACK,
        stack_size = const BOOT_STACK_SIZE,
        init_pt = sym init_boot_page_table,
        enable_mmu = sym axcpu::init::init_mmu,
        rust_entry = sym axplat::call_main,
    )
}
