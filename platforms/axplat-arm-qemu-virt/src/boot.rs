use core::{
    arch::naked_asm,
    ops::{Deref, DerefMut},
};

use axplat::mem::Aligned4K;
use memory_addr::{PhysAddr, pa};
use page_table_entry::arm::A32PTE;
use page_table_entry::{GenericPTE, MappingFlags};

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.boot_page_table")]
static mut BOOT_PT: Aligned4K<[A32PTE; 4096]> = Aligned4K::new([A32PTE::empty(); 4096]);

#[unsafe(link_section = ".bss.stack")]
static mut BOOT_STACK: Aligned4K<[u8; 4096]> = Aligned4K::new([0; 4096]);

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[allow(named_asm_labels)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "
    /* ARM32 Linux zImage header */
    .rept 8
    nop                             /* Occupies first 0x20 bytes */
    .endr
    b       {entry}             /* 0x20: Jump to main kernel entry */
    .word   0x016f2818              /* 0x24: zImage magic number */
    .word   0                       /* 0x28: Absolute load address (0 = unknown/relocated) */
    .word   0                       /* 0x2C: Image end address */
    .word   0x04030201              /* 0x30: Endianness flag */
    ",
    entry = sym _start_primary,
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[allow(named_asm_labels)]
unsafe extern "C" fn _start_primary() -> ! {
    // X0 = dtb
    core::arch::naked_asm!("
    // Save DTB and CPU ID
    mov r10, r2             // Save DTB
    mrc p15, 0, r11, c0, c0, 5 // Read MPIDR
    and r11, r11, #0xffffff // Get CPU ID (affinity levels)

    // Compute relocation delta: runtime_addr - linked_addr
0:
    adr r12, 0b
    ldr r3, =0b
    sub r12, r12, r3

    // Get runtime physical address of BOOT_PT (MMU off)
    ldr r0, ={BOOT_PT}
    add r0, r0, r12

    // Setup temporary boot stack (runtime relocated address)
    ldr r4, ={BOOT_STACK}
    add r4, r4, r12
    ldr r5, =4096
    add sp, r4, r5

    // r0 = BOOT_PT runtime address
    mov r6, r0
    bl {init_page_tables_before_mmu}

    // Enable MMU with BOOT_PT runtime address in r0
    mov r0, r6
    bl {init_mmu}

    // Switch to high virtual address execution before removing low mapping
    ldr r7, ={PHYS_VIRT_OFFSET}
    adr r8, 2f
    add r8, r8, r7
    bx r8
2:

    bl {init_page_tables_after_mmu}

    // Pass CPU ID and DTB to rust_main, then tail-jump to it.
    mov r0, r11
    mov r1, r10
    b {rust_main}

1:
    b 1b",
    BOOT_PT = sym BOOT_PT,
    BOOT_STACK = sym BOOT_STACK,
    PHYS_VIRT_OFFSET = const crate::config::plat::PHYS_VIRT_OFFSET,
    init_page_tables_before_mmu = sym init_page_tables_before_mmu,
    init_page_tables_after_mmu = sym init_page_tables_after_mmu,
    rust_main = sym axplat::call_main,
    init_mmu = sym axcpu::init::init_mmu,
    );
}

#[unsafe(no_mangle)]
unsafe extern "C" fn init_page_tables_before_mmu(boot_pt_runtime: usize) {
    let boot_pt_ptr = boot_pt_runtime as *mut A32PTE;
    unsafe {
        core::ptr::write_bytes(boot_pt_ptr, 0, 4096);
    }

    let section_flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE;
    let section_paddr = PhysAddr::from(0x4000_0000usize);
    let section_entry = A32PTE::new_section(section_paddr, section_flags);

    let uart_section_flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE;
    let uart_section_paddr = PhysAddr::from(0x0900_0000usize);
    let uart_section_entry = A32PTE::new_section(uart_section_paddr, uart_section_flags);

    unsafe {
        core::ptr::write_volatile(boot_pt_ptr.add(0x400), section_entry);
        core::ptr::write_volatile(boot_pt_ptr.add(0xC00), section_entry);
        core::ptr::write_volatile(boot_pt_ptr.add(0x890), uart_section_entry);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn init_page_tables_after_mmu() {
    unsafe {
        BOOT_PT[0x400] = A32PTE::empty();

        let device_flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE;
        for i in 0usize..128 {
            let pa_section_base = 0x0800_0000usize + (i << 20);
            let entry = A32PTE::new_section(pa!(pa_section_base), device_flags);

            BOOT_PT[0x880 + i] = entry;
        }

        let kernel_flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE;
        for i in 0usize..128 {
            let pa_section_base = 0x4000_0000usize + (i << 20);
            let entry = A32PTE::new_section(pa!(pa_section_base), kernel_flags);

            BOOT_PT[0xC00 + i] = entry;
        }
    }

    // axcpu::asm::dsb();
    // axcpu::asm::isb();

    uart_peuts("Page tables updated after MMU enabled.\n");
}

fn uart_early_putc(ch: u8) {
    unsafe {
        core::ptr::write_volatile(0x09000000 as *mut u8, ch);
    }
}

fn print_early_hex32(value: usize) {
    fn write_nibble(value: usize, shift: usize) {
        let digit = (value >> shift) & 0xf;
        let ascii = if digit < 10 {
            digit + b'0' as usize
        } else {
            digit - 10 + b'a' as usize
        };
        uart_early_putc(ascii as u8);
    }

    write_nibble(value, 28);
    write_nibble(value, 24);
    write_nibble(value, 20);
    write_nibble(value, 16);
    write_nibble(value, 12);
    write_nibble(value, 8);
    write_nibble(value, 4);
    write_nibble(value, 0);
}

#[inline]
fn uart_puch(ch: u8) {
    unsafe {
        core::ptr::write_volatile(0x89000000 as *mut u8, ch);
    }
}

fn uart_peuts(s: &str) {
    for &b in s.as_bytes() {
        uart_puch(b);
    }
}
