use crate::config::plat::{BOOT_STACK_SIZE, PHYS_VIRT_OFFSET};
use axcpu::asm::{dsb, isb};
use axplat::mem::{Aligned16K, pa};
use page_table_entry::{GenericPTE, MappingFlags, arm::A32PTE};

/// Boot page table for ARM32 short-descriptor format.
/// With TTBCR.N=1:
/// - TTBR0 covers 0x0000_0000 ~ 0x7FFF_FFFF (low 2GB, user space)
/// - TTBR1 covers 0x8000_0000 ~ 0xFFFF_FFFF (high 2GB, kernel space)
///
/// For simplicity during boot, we use a single unified page table for both.
/// The table has 4096 entries, each covering 1MB (total 4GB address space).
#[unsafe(no_mangle)]
#[unsafe(link_section = ".data")]
static mut BOOT_PT: Aligned16K<[A32PTE; 4096]> = Aligned16K::new([A32PTE::empty(); 4096]);

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
    // Number of 1MB sections for the temporary identity mapping
    const EARLY_BOOT_SECTION_NUM: usize = 4;

    // 1. Identity Map (Low 2GB - TTBR0 region):
    //    Temporarily map 0x4000_0000..0x403F_FFFF to itself.
    //    This keeps the early boot code and boot stacks accessible while the CPU
    //    is transitioning to the high virtual mapping right after enabling the MMU.
    for i in 0..EARLY_BOOT_SECTION_NUM {
        let paddr = 0x4000_0000 + i * 0x10_0000;
        let entry1 = A32PTE::new_page(
            pa!(paddr),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true, // 1MB section
        );
        unsafe { pt_ptr.add(paddr >> 20).write_volatile(entry1.bits() as u32) };
    }

    // 2. Kernel Linear Map (High 2GB - TTBR1 region):
    //    Temporarily map 0x4000_0000..0x403F_FFFF to 0xC000_0000..0xC03F_FFFF.
    //    This is enough for the early boot code and boot stacks before the full
    //    kernel linear mapping is installed later.
    let start_idx = 0xC000_0000 >> 20;
    for i in 0..EARLY_BOOT_SECTION_NUM {
        let paddr = 0x4000_0000 + i * 0x10_0000;
        let entry = A32PTE::new_page(
            pa!(paddr),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true, // 1MB section
        ); // Normal Memory Attributes
        unsafe {
            pt_ptr
                .add(start_idx + i)
                .write_volatile(entry.bits() as u32)
        };
    }
}

/// Map a new page after MMU is enabled.
/// This function is called after the MMU is turned on, so the `BOOT_PT`
/// is now accessible via its virtual address.
///
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn init_page_tables_after_mmu() {
    // SECTION_SIZE is 1MB for ARMv7-A short-descriptor format when using section entries.
    const SECTION_SIZE: usize = 0x10_0000;

    // Map all low memory (0..0x4000_0000) into 0x8000_0000..0xC000_0000
    // as device memory so phys_to_virt() can access any MMIO range without
    // depending on per-device boot-time mappings.
    for i in 0..0x4000_0000 / SECTION_SIZE {
        unsafe {
            BOOT_PT[0x800 + i] = A32PTE::new_page(
                pa!(i * SECTION_SIZE),
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
                true, // 1MB section
            );
        }
    }

    // Map the entire physical memory (0x4000_0000..0x8000_0000) into 0xC000_0000..0x1_0000_0000
    for i in 0..0x4000_0000 / SECTION_SIZE {
        unsafe {
            BOOT_PT[0xC00 + i] = A32PTE::new_page(
                pa!(0x4000_0000 + i * SECTION_SIZE),
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
                true, // 1MB section
            );
        }
    }

    // Flush TLB to ensure the new page table entries take effect immediately.
    axcpu::asm::flush_tlb(None);

    // Synchronization barriers using aarch32_cpu abstractions
    // These include compiler fences for proper ordering
    dsb();
    isb();
}

unsafe fn enable_fp() {
    #[cfg(feature = "fp-simd")]
    axcpu::asm::enable_fp();
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[allow(named_asm_labels)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "
        // Save DTB and CPU ID
        mov r10, r2             // Save DTB
        mrc p15, 0, r11, c0, c0, 5 // Read MPIDR
        and r11, r11, #0xffffff // Get CPU ID (affinity levels)

        // Calculate the physical offset
        // r3 = PHYS_VIRT_OFFSET
        ldr r3, ={PHYS_VIRT_OFFSET}

        // Setup Physical Stack
        ldr sp, ={BOOT_STACK}
        ldr r4, ={BOOT_STACK_SIZE}
        add sp, sp, r4
        sub sp, sp, r3

        // Enable FPU early
        bl {enable_fp}

        // Reload r3 as it might be clobbered by function calls
        ldr r3, ={PHYS_VIRT_OFFSET}

        // Get Physical Address of BOOT_PT
        ldr r0, ={BOOT_PT}
        sub r0, r0, r3

        // Call Rust function to setup page tables
        bl {init_page_tables}

        // Reload r3 as it might be clobbered by function calls
        ldr r3, ={PHYS_VIRT_OFFSET}

        // Call Rust function to initialize and enable MMU
        ldr r0, ={BOOT_PT}
        sub r0, r0, r3
        bl {init_mmu}

        // Jump to High Address
        ldr r2, =rust_entry_trampoline
        bx r2

    rust_entry_trampoline:
        // Setup Stack (Virtual Address)
        ldr sp, ={BOOT_STACK}
        ldr r3, ={BOOT_STACK_SIZE}
        add sp, sp, r3

        bl {init_page_tables_after_mmu}

        mov r0, r11 // cpu_id
        mov r1, r10 // dtb
        ldr r3, = {rust_main}
        bx r3
        b .",
        PHYS_VIRT_OFFSET = const PHYS_VIRT_OFFSET,
        BOOT_PT = sym BOOT_PT,
        BOOT_STACK = sym BOOT_STACK,
        BOOT_STACK_SIZE = const BOOT_STACK_SIZE,
        rust_main = sym axplat::call_main,
        init_page_tables = sym init_page_tables,
        init_mmu = sym axcpu::init::init_mmu,
        init_page_tables_after_mmu = sym init_page_tables_after_mmu,
        enable_fp = sym enable_fp,
    )
}

#[cfg(feature = "smp")]
#[unsafe(naked)]
#[allow(named_asm_labels)]
pub unsafe extern "C" fn _start_secondary() -> ! {
    core::arch::naked_asm!(
        "
        // r0 = physical stack pointer

        // Get CPU ID
        mrc p15, 0, r11, c0, c0, 5
        and r11, r11, #0xffffff

        // Setup physical stack
        mov sp, r0

        // Enable FPU
        bl {enable_fp}

        // Enable MMU
        ldr r3, ={PHYS_VIRT_OFFSET}
        ldr r0, ={BOOT_PT}
        sub r0, r0, r3
        bl {init_mmu}

        // Jump to trampoline
        ldr r2, =secondary_trampoline
        bx r2

    secondary_trampoline:
        // Switch stack to virtual
        ldr r3, ={PHYS_VIRT_OFFSET}
        add sp, sp, r3

        // Call secondary main
        mov r0, r11
        ldr r3, ={entry}
        bx r3
        b .",
        PHYS_VIRT_OFFSET = const PHYS_VIRT_OFFSET,
        BOOT_PT = sym BOOT_PT,
        init_mmu = sym axcpu::init::init_mmu,
        entry = sym axplat::call_secondary_main,
        enable_fp = sym enable_fp,
    )
}
