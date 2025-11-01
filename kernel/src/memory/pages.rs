use core::marker::PhantomData;

use bootloader_api::BootInfo;
use kernel_utils::no_heap_vec::NoHeapVec;
use log::{debug, info};
use x86_64::{
    VirtAddr,
    structures::paging::{PageSize, Size4KiB, Translate},
};

/// Represents a contiguous unmapped range in virtual space.
#[derive(Debug, Clone)]
struct FreeRegion {
    start: VirtAddr,
    end: VirtAddr, // exclusive
}

type FreeRegions<const MAX_REGIONS: usize> = NoHeapVec<MAX_REGIONS, FreeRegion>;

pub struct VirtRegionAllocator<const MAX_REGIONS: usize = 256, S: PageSize = Size4KiB> {
    free_regions: FreeRegions<MAX_REGIONS>,
    current: usize,
    cursor: VirtAddr,
    page_size: PhantomData<S>,
}

impl<const MAX_REGIONS: usize, S: PageSize> core::fmt::Debug
    for VirtRegionAllocator<MAX_REGIONS, S>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VirtRegionAllocator")
            .field("free_regions", &self.free_regions)
            .field("current", &self.current)
            .field("cursor", &self.cursor)
            .field("page_size", &S::DEBUG_STR)
            .finish()
    }
}

impl<S: PageSize> VirtRegionAllocator<1, S> {
    /// Scans [start, end) for unmapped pages and builds a bump allocator.
    pub fn new_empty(start: VirtAddr, end: VirtAddr) -> Self {
        let free_regions = FreeRegions::from([FreeRegion { start, end }]);
        let cursor = start;
        Self {
            free_regions,
            current: 0,
            cursor,
            page_size: PhantomData,
        }
    }
}

impl<const MAX_REGIONS: usize, S: PageSize> VirtRegionAllocator<MAX_REGIONS, S> {
    /// Scans [start, end) for unmapped pages and builds a bump allocator.
    pub fn new<T: Translate>(translator: &T, start: VirtAddr, end: VirtAddr) -> Self {
        let free_regions = Self::scan_unmapped(translator, start, end);
        let cursor = free_regions.first().map(|r| r.start).unwrap_or(start);
        Self {
            free_regions,
            current: 0,
            cursor,
            page_size: PhantomData,
        }
    }

    /// Find contiguous unmapped ranges.
    fn scan_unmapped<T: Translate>(
        translator: &T,
        start: VirtAddr,
        end: VirtAddr,
    ) -> FreeRegions<MAX_REGIONS> {
        debug!("Performing region scan");
        let mut regions = FreeRegions::new();
        let mut cur = start.align_up(S::SIZE);
        let mut run_start: Option<VirtAddr> = None;

        while cur < end {
            let mapped = translator.translate_addr(cur).is_some();

            match (mapped, run_start) {
                (false, None) => run_start = Some(cur),
                (true, Some(s)) => {
                    debug!("Found empty region: {s:p} -> {cur:p}");
                    if regions.push(FreeRegion { start: s, end: cur }).is_err() {
                        return regions; // Early exit if full
                    }
                    run_start = None;
                }
                _ => {}
            }

            cur += S::SIZE;
        }

        if let Some(s) = run_start {
            debug!("Found empty region: {s:p} -> {end:p}");
            let _ = regions.push(FreeRegion { start: s, end }); // Ignore if full
        }

        regions
    }

    /// Allocate `num_pages` contiguous 4 KiB pages (aligned to page boundary).
    pub fn alloc_pages(&mut self, num_pages: usize) -> Option<VirtAddr> {
        let size = num_pages as u64 * S::SIZE;

        loop {
            if self.current >= self.free_regions.len() {
                return None; // out of space
            }

            let region = &mut self.free_regions[self.current];
            let next_cursor = self.cursor + size;

            if next_cursor <= region.end {
                let alloc_start = self.cursor;
                self.cursor = next_cursor;
                return Some(alloc_start);
            } else {
                // move to next region
                self.current += 1;
                if let Some(next) = self.free_regions.get(self.current) {
                    self.cursor = next.start;
                }
            }
        }
    }
}

/// Initialize a small region allocator on top of BootInfo-derived free_start.
///
/// This creates a simple 1 GiB virtual window starting at `layout.free_start`.
/// The allocator we create is a very small "virtual-only" bump allocator: it returns
/// virtual addresses (unmapped) that your subsystems can map later to physical frames.
///
/// You can change `DEFAULT_WINDOW_BYTES` to a smaller/larger value depending on needs.
pub fn init_region_allocator<S: PageSize, T: Translate>(
    layout: &KernelVirtLayout,
    translator: &T,
) -> VirtRegionAllocator<1, S> {
    // default window: 1 GiB
    const DEFAULT_WINDOW_BYTES: u64 = /* 1 * */ 1024 * 1024 * 1024u64;
    const PROBE_STEP: u64 = 2 * 1024 * 1024; // 2 MiB sparse probing
    let align = S::SIZE.max(2 * 1024 * 1024); // align to page-size or 2MiB for speed

    let candidate_start = layout.free_start.align_up(align);
    let (start, end) = find_free_window(
        translator,
        candidate_start,
        DEFAULT_WINDOW_BYTES,
        PROBE_STEP,
        align,
    )
    .expect("no free virtual window found");

    info!(
        "virt region allocator window: {:#x} - {:#x}",
        start.as_u64(),
        end.as_u64()
    );
    VirtRegionAllocator::new_empty(start, end)
}

/// Information about your kernel’s virtual layout.
#[derive(Debug)]
pub struct KernelVirtLayout {
    pub kernel: (VirtAddr, VirtAddr),
    pub framebuffer: Option<(VirtAddr, VirtAddr)>,
    pub free_start: VirtAddr,
}

/// Derive a safe starting virtual address for new allocations.
pub fn discover_layout(boot_info: &BootInfo) -> KernelVirtLayout {
    // Compute kernel virtual range
    let kernel_start = VirtAddr::new(boot_info.kernel_image_offset);
    let kernel_end = kernel_start + boot_info.kernel_len;

    // Check framebuffer (may not exist)
    let framebuffer = boot_info.framebuffer.as_ref().map(|fb| {
        let buffer_start = fb.buffer().as_ptr();
        (
            VirtAddr::from_ptr(buffer_start),
            VirtAddr::new(buffer_start as u64 + fb.info().byte_len as u64),
        )
    });

    // Highest occupied virtual address among known explicit regions
    let mut highest = kernel_end;
    if let Some((_, end)) = framebuffer
        && end > highest
    {
        highest = end;
    }

    // If the bootloader mapped the entire physical address space at some virtual offset,
    // include that mapped range in the occupied set so we don't allocate into it.
    if let Some(offset) = boot_info.physical_memory_offset.as_ref() {
        // Find the highest physical address reported in the memory regions
        let mut highest_phys_end: u64 = 0;
        for region in boot_info.memory_regions.iter() {
            let end = region.end;
            if end > highest_phys_end {
                highest_phys_end = end;
            }
        }

        // Virtual end of the physical-memory mapping:
        // phys_map_virt_end = offset + highest_phys_end
        let phys_map_virt_end = VirtAddr::new(offset + highest_phys_end);
        if phys_map_virt_end > highest {
            highest = phys_map_virt_end;
        }
    }

    // NOTE: If you load modules via the bootloader, consider them too:
    // for module in boot_info.modules.iter() { ... } // include their virt end in `highest`.
    // (I didn't add modules usage here because BootInfo shape in your snippet didn't include them,
    //  but if you do use bootloader modules, include them the same way as framebuffer.)

    // Round up to 2 MiB boundary for cleanliness (nice for large pages)
    let free_start = highest.align_up(0x20_0000u64);

    KernelVirtLayout {
        kernel: (kernel_start, kernel_end),
        framebuffer,
        free_start,
    }
}

// Probe the candidate window [start, start + window_bytes) and return the first
/// start aligned to `align` such that the whole window appears unmapped.
///
/// - `translator`: something implementing `Translate` (e.g. your OffsetPageTable)
/// - `probe_step`: how far between coarse probes (use 2 MiB for speed)
fn find_free_window<T: Translate>(
    translator: &T,
    mut start: VirtAddr,
    window_bytes: u64,
    probe_step: u64,
    align: u64,
) -> Option<(VirtAddr, VirtAddr)> {
    // align start
    start = start.align_up(align);

    loop {
        let end = start + window_bytes;
        // quick out-of-range check — nobody should ask for absurd values
        if end.as_u64() < start.as_u64() {
            return None;
        }

        // Coarse-scan: check one address every `probe_step`.
        let mut bad = false;
        let mut probe = start;
        while probe < end {
            if translator.translate_addr(probe).is_some() {
                // found a mapped page inside the window -> skip past it
                // advance start to just beyond this mapped probe, aligned up
                start = (probe + probe_step).align_up(align);
                bad = true;
                break;
            }
            probe += probe_step;
        }

        if bad {
            // try again with new start
            continue;
        }

        // Fine-grained sanity: check a small region at start and end (optional)
        // For safety you can also check every 4 KiB in the first/last 2 MiB
        return Some((start, end));
    }
}
