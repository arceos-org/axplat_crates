use crate::config::{BOOT_STACK_SIZE, KERNEL_BASE, PHY_MEM_BASE};
use core::arch::asm;
use memory_addr::pa;
use page_table_entry::{GenericPTE, MappingFlags, arm::A32PTE};

/// Boot page table for ARM32 short-descriptor format.
/// With TTBCR.N=1:
/// - TTBR0 covers 0x0000_0000 ~ 0x7FFF_FFFF (low 2GB, user space)
/// - TTBR1 covers 0x8000_0000 ~ 0xFFFF_FFFF (high 2GB, kernel space)
///
/// For simplicity during boot, we use a single unified page table for both.
/// The table has 4096 entries, each covering 1MB (total 4GB address space).
#[repr(C, align(16384))]
struct BootPageTable {
    entries: [u32; 4096],
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.boot_page_table")]
static mut BOOT_PT: BootPageTable = BootPageTable { entries: [0; 4096] };

#[unsafe(no_mangle)]
#[unsafe(link_section = ".bss.stack")]
static mut BOOT_STACK: [u8; BOOT_STACK_SIZE] = [0; BOOT_STACK_SIZE];

/// Initialize the boot page tables.
///
/// This function is called from the assembly startup code (`_start`).
/// The MMU is not yet enabled, so we must operate on physical addresses directly.
/// `pt_ptr` is the *physical* address of the `BOOT_PT` page table.
///
/// With TTBCR.N=1 configuration:
/// - Low 2GB (0x0000_0000 ~ 0x7FFF_FFFF): Used by TTBR0 (user/identity mappings)
/// - High 2GB (0x8000_0000 ~ 0xFFFF_FFFF): Used by TTBR1 (kernel mappings)
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn init_page_tables(pt_ptr: *mut u32) {
    // 1. Identity Map (Low 2GB - TTBR0 region):
    //    Mapping physical RAM (PHY_MEM_BASE = 0x4000_0000) to itself.
    //    This is required so the CPU can keep executing instructions immediately
    //    after turning on the MMU (where PC is still pointing to physical addresses).
    let entry1 = A32PTE::new_page(
        pa!(PHY_MEM_BASE),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true, // 1MB section
    );
    unsafe {
        pt_ptr
            .add(PHY_MEM_BASE >> 20)
            .write_volatile(entry1.bits() as u32)
    };

    // 2. Kernel Linear Map (High 2GB - TTBR1 region):
    //    Map physical RAM (PHY_MEM_BASE) to KERNEL_BASE in high memory.
    //    Virtual Range: KERNEL_BASE (0xC000_0000) -> 0xFFFF_FFFF (TTBR1 region)
    //    Physical Range: PHY_MEM_BASE (0x4000_0000) -> 0x4640_0000
    let start_idx = KERNEL_BASE >> 20;
    // Only map 1MB for Kernel Code & Data mainly.

    let entry = A32PTE::new_page(
        pa!(PHY_MEM_BASE),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true, // 1MB section
    ); // Normal Memory Attributes
    unsafe { pt_ptr.add(start_idx).write_volatile(entry.bits() as u32) };
}

/// Map a new page after MMU is enabled.
/// This function is called after the MMU is turned on, so the `BOOT_PT`
/// is now accessible via its virtual address.
///
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn init_page_tables_after_mmu() {
    // Get the virtual address of the page table
    // BOOT_PT is in the high memory after MMU is enabled
    let pt_vaddr = core::ptr::addr_of_mut!(BOOT_PT) as *mut u32;

    // Create the page table entry
    let entry = A32PTE::new_page(
        pa!(0x0900_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        true, // 1MB section
    );

    // Write the entry to the page table
    unsafe {
        pt_vaddr
            .add(0x8900_0000 >> 20)
            .write_volatile(entry.bits() as u32);
    }

    // 2. Kernel Linear Map (High 1GB - TTBR1 region):
    let start_idx = KERNEL_BASE >> 20;
    // Only map 1MB for Kernel Code & Data mainly.

    for idx in 1..0x1000 {
        let entry = A32PTE::new_page(
            pa!(PHY_MEM_BASE + (idx * 0x0010_0000)),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true, // 1MB section
        ); // Normal Memory Attributes
        unsafe { pt_vaddr.add(start_idx + idx).write_volatile(entry.bits() as u32) };
    }

    // Flush TLB to ensure the new mapping takes effect
    unsafe {
        asm!("mcr p15, 0, {}, c8, c7, 0", in(reg) 0u32); // TLBIALL
        asm!("dsb");
        asm!("isb");
    }
}

/// Initialize and enable the MMU with user/kernel address space separation.
///
/// This function configures the MMU with TTBCR.N=1 to split the address space:
/// - TTBR0: Low 2GB (0x0000_0000 ~ 0x7FFF_FFFF) - User space
/// - TTBR1: High 2GB (0x8000_0000 ~ 0xFFFF_FFFF) - Kernel space
///
/// For boot simplicity, both TTBR0 and TTBR1 point to the same page table,
/// which contains mappings for both address ranges.
///
/// # Arguments
/// * `root_paddr` - Physical address of the unified page table
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn init_mmu(root_paddr: usize) {
    let root = root_paddr as u32;

    unsafe {
        // Set TTBR0 (Translation Table Base Register 0) - for low 2GB
        // Used for addresses 0x0000_0000 ~ 0x7FFF_FFFF
        asm!("mcr p15, 0, {}, c2, c0, 0", in(reg) root);

        // Set TTBR1 (Translation Table Base Register 1) - used for kernel space > 2GB
        asm!("mcr p15, 0, {}, c2, c0, 1", in(reg) root);

        // Set TTBCR.N=1 to split the address space at 2GB.
        // TTBR0: 0x0000_0000 ~ 0x7FFF_FFFF
        // TTBR1: 0x8000_0000 ~ 0xFFFF_FFFF
        asm!("mcr p15, 0, {}, c2, c0, 2", in(reg) 1u32);

        // Set Domain Access Control Register (all domains to client mode)
        // Domain 0-15: 01 = Client (check page table permissions)
        asm!("mcr p15, 0, {}, c3, c0, 0", in(reg) 0x55555555u32);

        // Invalidate TLB to ensure we don't have stale entries
        asm!("mcr p15, 0, {}, c8, c7, 0", in(reg) 0u32);

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

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "
        // Calculate the physical offset
        // r3 = KERNEL_OFFSET (0x4000_0000)
        ldr r4, ={KERNEL_BASE}
        ldr r5, ={PHY_MEM_BASE}
        sub r3, r4, r5

        // Setup Physical Stack
        ldr sp, =BOOT_STACK
        ldr r4, ={BOOT_STACK_SIZE}
        add sp, sp, r4
        sub sp, sp, r3

        // Get Physical Address of BOOT_PT
        ldr r0, =BOOT_PT
        sub r0, r0, r3

        // Call Rust function to setup page tables
        bl {init_page_tables}

        // Call Rust function to initialize and enable MMU
        ldr r0, =BOOT_PT
        sub r0, r0, r3
        bl {init_mmu}

        // Jump to High Address
        ldr r2, =rust_entry_trampoline
        bx r2

    rust_entry_trampoline:
        // Unmap the identity mapping (PHY_MEM_BASE)
        // r0 = Virtual Address of BOOT_PT (since we are now in high address)
        ldr r0, =BOOT_PT
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
        ldr sp, =BOOT_STACK
        ldr r3, ={BOOT_STACK_SIZE}
        add sp, sp, r3

        bl {init_page_tables_after_mmu}

        bl {rust_main}
        b .",
        KERNEL_BASE = const KERNEL_BASE,
        PHY_MEM_BASE = const PHY_MEM_BASE,
        BOOT_STACK_SIZE = const BOOT_STACK_SIZE,
        rust_main = sym crate::rust_main,
        init_page_tables = sym init_page_tables,
        init_mmu = sym init_mmu,
        init_page_tables_after_mmu = sym init_page_tables_after_mmu,
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
        init_mmu = sym axcpu::init::init_mmu,
        entry = sym axplat::call_secondary_main,
    )
}
