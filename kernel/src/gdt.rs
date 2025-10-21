use spin::Lazy;
use x86_64::{
    VirtAddr,
    instructions::interrupts,
    registers::segmentation::{DS, ES, FS, GS, SS},
    structures::{
        gdt::{Descriptor, DescriptorFlags, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

use crate::{println, stack::SlabStack};

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
    let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
    let user_code_selector = gdt.append(Descriptor::user_code_segment());
    let user_data_selector = gdt.append(Descriptor::user_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
    (
        gdt,
        Selectors {
            kernel_code_selector,
            kernel_data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
        },
    )
});

pub struct Selectors {
    pub kernel_code_selector: SegmentSelector,
    pub kernel_data_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, Segment};
    use x86_64::instructions::tables::load_tss;
    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.kernel_code_selector);
        DS::set_reg(GDT.1.kernel_data_selector);
        ES::set_reg(GDT.1.kernel_data_selector);
        FS::set_reg(GDT.1.kernel_data_selector);
        GS::set_reg(GDT.1.kernel_data_selector);
        SS::set_reg(GDT.1.kernel_data_selector);
        load_tss(GDT.1.tss_selector);
    }
}

pub fn selectors() -> &'static Selectors {
    &GDT.1
}

pub extern "C" fn kernel_code_selector() -> u64 {
    let idx = selectors().kernel_code_selector.0 >> 3; // index in GDT
    let desc = &GDT.0.entries()[idx as usize];
    println!(
        "Kernel code descriptor: {:?} {:?}",
        desc,
        DescriptorFlags::from_bits(desc.raw())
    );
    let s = selectors().kernel_code_selector.0 as u64;
    println!("Kernel CS: {s:x}");
    s
}

pub fn set_tss_guarded_stacks(esp0: SlabStack, ist_df: SlabStack) {
    interrupts::disable();

    println!("Setting guarded stacks for TSS");
    println!(" - ESP0 = {:p}", esp0.top());
    println!(" - IST_DF = {:p}", ist_df.top());

    let tss_mut = unsafe { TSS.as_mut_ptr().as_mut() }.unwrap();
    tss_mut.privilege_stack_table[0] = esp0.top();
    tss_mut.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = ist_df.top();

    interrupts::enable();
}

pub fn get_esp0_stack_top() -> VirtAddr {
    TSS.privilege_stack_table[0]
}
