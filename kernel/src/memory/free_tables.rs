use x86_64::structures::paging::PageTableIndex;

pub struct FreeEntry {
    pub index: PageTableIndex,
    pub alignment: u64,
}

pub trait FreeTables {
    fn free_l4_kernel_entries(&self) -> impl Iterator<Item = FreeEntry>;
}
