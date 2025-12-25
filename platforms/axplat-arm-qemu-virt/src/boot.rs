//! Early boot initialization code for ARMv7-A.

use axplat::mem::pa;
use page_table_entry::{GenericPTE, MappingFlags, arm::A32PTE};

use crate::config::plat::BOOT_STACK_SIZE;

/// Boot stack, 256KB
#[unsafe(link_section = ".bss.stack")]
pub static mut BOOT_STACK: [u8; BOOT_STACK_SIZE] = [0; BOOT_STACK_SIZE];

/// Compile-time constants for loading BOOT_STACK_SIZE into ARM registers
/// Split into low and high 16 bits for movw/movt instructions
const BOOT_STACK_SIZE_LOW: u16 = (BOOT_STACK_SIZE & 0xFFFF) as u16;
const BOOT_STACK_SIZE_HIGH: u16 = ((BOOT_STACK_SIZE >> 16) & 0xFFFF) as u16;

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
        // QEMU virt machine memory layout (with -m 128M):
        // - 0x00000000..0x08000000: Unmapped/reserved
        // - 0x08000000..0x40000000: MMIO devices (Flash, UART, GIC, PCIe, etc.)
        // - 0x40000000..0x48000000: 128MB RAM
        
        // 0x0000_0000..0x0800_0000 (0-128MB): Identity map as device memory for safety
        // This prevents accidental access to undefined regions
        for i in 0..0x80 {
            BOOT_PT_L1.0[i] = A32PTE::new_page(
                pa!(i * 0x10_0000),
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
                true,
            );
        }

        // 0x0800_0000..0x4000_0000 (128MB-1GB): Device memory (MMIO)
        // Includes: Flash, PL011 UART, PL031 RTC, GICv2, PCIe, VirtIO devices
        for i in 0x80..0x400 {
            BOOT_PT_L1.0[i] = A32PTE::new_page(
                pa!(i * 0x10_0000),
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
                true,
            );
        }

        // 0x4000_0000..0x4800_0000 (1GB-1GB+128MB): RAM, RWX
        // This is where QEMU loads the kernel and provides working memory
        for i in 0x400..0x480 {
            BOOT_PT_L1.0[i] = A32PTE::new_page(
                pa!(i * 0x10_0000),
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
                true, // 1MB section
            );
        }

        // 0x4800_0000..0x1_0000_0000 (1GB+128MB-4GB): Unmapped or device memory
        // Map remaining space as device memory to avoid faults
        for i in 0x480..0x1000 {
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
        // Use movw/movt to load BOOT_STACK_SIZE (split into 16-bit parts)
        ldr sp, ={boot_stack}
        movw r0, #{stack_size_low}     // Lower 16 bits of BOOT_STACK_SIZE
        movt r0, #{stack_size_high}    // Upper 16 bits of BOOT_STACK_SIZE
        add sp, sp, r0
        
        // Initialize page table
        bl {init_pt}
        
        // Enable MMU
        ldr r0, ={boot_pt}              // Use ldr= pseudo-instruction for full 32-bit address
        bl {enable_mmu}
        
        // Jump to Rust entry
        bl {rust_entry}
    1:  b 1b",
        boot_stack = sym BOOT_STACK,
        boot_pt = sym BOOT_PT_L1,
        stack_size_low = const BOOT_STACK_SIZE_LOW,
        stack_size_high = const BOOT_STACK_SIZE_HIGH,
        init_pt = sym init_boot_page_table,
        enable_mmu = sym axcpu::init::init_mmu,
        rust_entry = sym axplat::call_main,
    )
}

/// The earliest entry point for the secondary CPUs.
#[cfg(feature = "smp")]
#[unsafe(naked)]
pub(crate) unsafe extern "C" fn _start_secondary() -> ! {
    // R0 = stack pointer (passed from primary CPU)
    core::arch::naked_asm!(
        "
        // Save stack pointer from R0
        mov sp, r0
        
        // Get CPU ID from MPIDR
        mrc p15, 0, r4, c0, c0, 5       // Read MPIDR
        and r4, r4, #0xff               // Extract CPU ID (Aff0)
        
        // Enable MMU (page table already initialized by primary CPU)
        ldr r0, ={boot_pt}              // Use ldr= for full 32-bit address
        bl {init_mmu}
        
        // Call secondary main entry with CPU ID
        mov r0, r4                      // Pass CPU ID as argument
        ldr r1, ={entry}
        blx r1                          // Use blx (not blr, that's AArch64)
    1:  b 1b",
        boot_pt = sym BOOT_PT_L1,
        init_mmu = sym axcpu::init::init_mmu,
        entry = sym axplat::call_secondary_main,
    )
}
