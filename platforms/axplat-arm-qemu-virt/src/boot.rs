//! Early boot initialization code for ARMv7-A.

use axplat::mem::pa;
use page_table_entry::{GenericPTE, MappingFlags, arm::A32PTE};

use crate::config::{devices::UART_PADDR, plat::BOOT_STACK_SIZE};

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
static mut BOOT_PT: Aligned16K<[A32PTE; 4096]> = Aligned16K::new([A32PTE::empty(); 4096]);

/// Initialize boot page table.
/// This function is unsafe as it modifies global static variables.
#[unsafe(no_mangle)]
pub unsafe fn init_boot_page_table(pt_ptr: *mut u32) {
    // 1. Identity Map (Low 2GB - TTBR0 region):
    //    Mapping physical RAM (PHY_MEM_BASE = 0x4000_0000) to itself.
    //    This is required so the CPU can keep executing instructions immediately
    //    after turning on the MMU (where PC is still pointing to physical addresses).
    let entry1 = A32PTE::new_page(
        pa!(0x4000_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true, // 1MB section
    );
    unsafe { pt_ptr.add(0x4000_0000 >> 20).write_volatile(entry1.bits() as u32) };

    // 2. Kernel Linear Map (High 2GB - TTBR1 region):
    //    Map physical RAM (PHY_MEM_BASE) to KERNEL_BASE in high memory.
    //    Virtual Range: KERNEL_BASE (0xC000_0000) -> 0xFFFF_FFFF (TTBR1 region)
    //    Physical Range: PHY_MEM_BASE (0x4000_0000) -> 0x4640_0000 (99MB)
    let start_idx = 0xC000_0000 >> 20;
    for i in 0..99 {
        // 99 entries * 1MB
        let entry = A32PTE::new_page(
            pa!(0x4000_0000 + (i * 0x10_0000)),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true, // 1MB section
        ); // Normal Memory Attributes
        unsafe {
            pt_ptr
                .add(start_idx + i)
                .write_volatile(entry.bits() as u32)
        };
    }

    // 3. UART Map (High 2GB - TTBR1 region, Kernel space):
    //    Map UART physical address (0x0900_0000) to kernel virtual address (0x0900_0000).
    //    This allows early printk/logging to work even after MMU is on.
    //    UART is mapped in kernel space, not accessible to user space.
    let entry3 = A32PTE::new_page(
        pa!(UART_PADDR),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        true, // 1MB section
    );
    unsafe { pt_ptr.add(UART_PADDR >> 20).write_volatile(entry3.bits() as u32) };
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn _start() -> !{
    core::arch::naked_asm!(
        "
        // Calculate the physical offset
        // r3 = KERNEL_OFFSET (0x4000_0000)
        ldr r4, ={KERNEL_BASE}
        ldr r5, ={PHY_MEM_BASE}
        sub r3, r4, r5

        // Setup Physical Stack
        ldr sp, ={boot_stack}
        ldr r4, ={BOOT_STACK_SIZE}
        add sp, sp, r4
        sub sp, sp, r3

        // Get Physical Address of BOOT_PT
        ldr r0, ={boot_pt}
        sub r0, r0, r3

        // Call Rust function to setup page tables
        bl {init_page_tables}

        // Call Rust function to initialize and enable MMU
        ldr r0, ={boot_pt}
        sub r0, r0, r3
        bl {init_mmu}

        // Jump to High Address
        ldr r2, =rust_entry_trampoline
        bx r2

    rust_entry_trampoline:
        // Unmap the identity mapping (PHY_MEM_BASE)
        // r0 = Virtual Address of BOOT_PT (since we are now in high address)
        ldr r0, ={boot_pt}
        mov r1, #0
        // Index = 0x4000_0000 >> 20 = 0x400
        mov r4, #0x400
        str r1, [r0, r4, lsl #2] // Clear the entry

        // Invalidate TLB (Since we changed the page table)
        mov r0, #0
        mcr p15, 0, r0, c8, c7, 0 // TLBIALL
        dsb
        isb

        // Setup Stack (Virtual Address)
        ldr sp, ={boot_stack}
        ldr r3, ={BOOT_STACK_SIZE}
        add sp, sp, r3

        bl {rust_main}
        b .",
        KERNEL_BASE = const KERNEL_BASE,
        PHY_MEM_BASE = const PHY_MEM_BASE,
        BOOT_STACK_SIZE = const BOOT_STACK_SIZE,
        rust_main = sym crate::rust_main,
        init_page_tables = sym init_page_tables,
        init_mmu = sym init_mmu,
        boot_stack = sym BOOT_STACK,
        boot_pt = sym BOOT_PT,
    )
}

pub unsafe fn init_mmu(root_paddr: memory_addr::PhysAddr) {
    use core::arch::asm;

    let root = root_paddr.as_usize() as u32;

    unsafe {
        // Set TTBR0 (Translation Table Base Register 0) - for low 2GB
        // Used for addresses 0x0000_0000 ~ 0x7FFF_FFFF
        asm!("mcr p15, 0, {}, c2, c0, 0", in(reg) root); // TTBR0

        // Set TTBR1 (Translation Table Base Register 1) - for high 2GB
        // Used for addresses 0x8000_0000 ~ 0xFFFF_FFFF
        // During boot, we use the same page table for simplicity
        asm!("mcr p15, 0, {}, c2, c0, 1", in(reg) root); // TTBR1

        // Set TTBCR.N=1 to enable address space split at 2GB boundary
        // Bits [2:0] = N = 1:
        //   - TTBR0 for VA[31:31] = 0 (addresses < 0x8000_0000)
        //   - TTBR1 for VA[31:31] = 1 (addresses >= 0x8000_0000)
        asm!("mcr p15, 0, {}, c2, c0, 2", in(reg) 1u32); // TTBCR

        // Set Domain Access Control Register (all domains to client mode)
        // Domain 0-15: 01 = Client (check page table permissions)
        asm!("mcr p15, 0, {}, c3, c0, 0", in(reg) 0x55555555u32);

        // Data Synchronization Barrier
        asm!("dsb");

        // Instruction Synchronization Barrier
        asm!("isb");

        // Read SCTLR (System Control Register)
        let mut sctlr: u32;
        asm!("mrc p15, 0, {}, c1, c0, 0", out(reg) sctlr);

        // Enable MMU (M bit), data cache (C bit), instruction cache (I bit)
        sctlr |= (1 << 0) | (1 << 2) | (1 << 12);

        // Write back SCTLR
        asm!("mcr p15, 0, {}, c1, c0, 0", in(reg) sctlr);

        // Synchronization barriers
        asm!("dsb");
        asm!("isb");
    }
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

        // Calculate the physical offset
        // r3 = KERNEL_OFFSET (0x4000_0000)
        ldr r4, ={KERNEL_BASE}
        ldr r5, ={PHY_MEM_BASE}
        sub r3, r4, r5
        
        // Get CPU ID from MPIDR
        mrc p15, 0, r4, c0, c0, 5       // Read MPIDR
        and r4, r4, #0xff               // Extract CPU ID (Aff0)
        
        // Enable MMU (page table already initialized by primary CPU)
        ldr r0, ={boot_pt}              // Get virtual address of BOOT_PT
        sub r0, r0, r3                  // Convert to physical address
        bl {init_mmu}
        
        // Call secondary main entry with CPU ID
        mov r0, r4                      // Pass CPU ID as argument
        ldr r1, ={entry}
        blx r1                          // Use blx (not blr, that's AArch64)
    1:  b 1b",
        KERNEL_BASE = const KERNEL_BASE,
        PHY_MEM_BASE = const PHY_MEM_BASE,
        boot_pt = sym BOOT_PT,
        init_mmu = sym init_mmu,
        entry = sym axplat::call_secondary_main,
    )
}
