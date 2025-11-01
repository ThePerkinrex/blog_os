OUTPUT_FORMAT("elf64-x86-64")
OUTPUT_ARCH(i386:x86-64)
ENTRY(main)

PHDRS {
  text PT_LOAD FLAGS(5);    /* R + X */
  data PT_LOAD FLAGS(6);    /* R + W */
}

SECTIONS {
    . = 0x100000;

    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.got*)
        *(.got.plt*)
        *(.dynamic)
        *(.dynsym)
        *(.dynstr)
        *(.gnu.hash)
        *(.rela*)
        *(.interp)
    }

    .text : ALIGN(4K) {
        *(.text .text.*)
    } :text

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    } :text

    .eh_frame_hdr : ALIGN(8) {
        *(.eh_frame_hdr)
    } :text

    .eh_frame : ALIGN(8) {
        *(.eh_frame)
        *(.eh_frame.*)
    } :text

    .data : ALIGN(4K) {
        *(.data .data.*)
    } :data

    .bss : ALIGN(4K) {
        *(.bss .bss.*)
        *(COMMON)
    } :data
}
