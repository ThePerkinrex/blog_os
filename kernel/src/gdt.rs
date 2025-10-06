use spin::Lazy;
use x86_64::{
    VirtAddr,
    instructions::interrupts,
    registers::segmentation::{DS, ES, FS, GS, SS},
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

use crate::stack::Stack;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();

    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: u64 = 4096; // Really small stack, just for init
        static mut STACK: [u8; STACK_SIZE as usize] = [0; STACK_SIZE as usize];

        let stack_start = VirtAddr::from_ptr(&raw const STACK);

        stack_start + STACK_SIZE // stack_end
    };
    tss
});

static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.append(Descriptor::kernel_code_segment());
    let data_selector = gdt.append(Descriptor::kernel_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
    (
        gdt,
        Selectors {
            code_selector,
            data_selector,
            tss_selector,
        },
    )
});

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, Segment};
    use x86_64::instructions::tables::load_tss;
    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);

        DS::set_reg(GDT.1.data_selector);
        ES::set_reg(GDT.1.data_selector);
        FS::set_reg(GDT.1.data_selector);
        GS::set_reg(GDT.1.data_selector);
        SS::set_reg(GDT.1.data_selector);
    }
}

pub fn set_tss_guarded_stacks(esp0: Stack, ist_df: Stack) {
    interrupts::disable();

    let tss_mut = unsafe { TSS.as_mut_ptr().as_mut() }.unwrap();
    tss_mut.privilege_stack_table[0] = esp0.top();
    tss_mut.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = ist_df.top();

    interrupts::enable();
}
