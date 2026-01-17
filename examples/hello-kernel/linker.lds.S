ENTRY(_start)
SECTIONS
{
    . = %KERNEL_BASE%;
    _phys_base = %PHYS_BASE%;
    _skernel = .;

    .text : AT(_phys_base + ADDR(.text) - _skernel) ALIGN(4K) {
        _stext = .;
        *(.text.boot)
        *(.text .text.*)
        _etext = .;
    }

    .rodata : AT(_phys_base + ADDR(.rodata) - _skernel) ALIGN(4K) {
        _srodata = .;
        *(.rodata .rodata.*)
        _erodata = .;
    }

    .data : AT(_phys_base + ADDR(.data) - _skernel) ALIGN(4K) {
        _sdata = .;
        *(.data .data.*)
        *(.got .got.*)
    }

    . = ALIGN(4K);
    _percpu_start = .;
    _percpu_end = _percpu_start + SIZEOF(.percpu);
    .percpu 0x0 : AT(_percpu_start) {
        _percpu_load_start = .;
        *(.percpu .percpu.*)
        _percpu_load_end = .;
        . = _percpu_load_start + ALIGN(64) * 1;
    }
    . = _percpu_end;
    _edata = .;

    .bss : AT(_phys_base + ADDR(.bss) - _skernel) ALIGN(4K) {
        *(.bss.stack)
        . = ALIGN(4K);
        _sbss = .;
        *(.bss .bss.*)
        *(COMMON)
        _ebss = .;
    }

    _ekernel = .;

    /DISCARD/ : {
        *(.comment)
    }
}
