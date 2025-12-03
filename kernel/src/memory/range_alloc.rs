use core::{
    iter::Sum,
    ops::{Add, AddAssign, Mul, Range, Sub},
};

use bootloader_api::BootInfo;
use log::{debug, info};
use smallvec::SmallVec;
use x86_64::{
    VirtAddr,
    structures::paging::{PageSize, Translate},
};

use crate::{memory::free_tables::FreeTables, setup::KERNEL_INFO};

#[derive(Debug)]
pub struct RangeAllocator<T, const N: usize = 256> {
    /// The range this allocator covers.
    initial_range: Range<T>,
    /// A Vec of ranges in this heap which are unused.
    /// Must be ordered with ascending range start to permit short circuiting allocation.
    /// No two ranges in this vec may overlap.
    free_ranges: SmallVec<[Range<T>; N]>,
    alignment: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeAllocationError<T> {
    pub fragmented_free_length: T,
}

impl<const N: usize, T> RangeAllocator<T, N>
where
    T: Clone
        + Copy
        + Add<Output = T>
        + AddAssign
        + Sub<Output = T>
        + Mul<Output = T>
        + Eq
        + PartialOrd
        + core::fmt::Debug,
{
    pub fn new(range: Range<T>, alignment: T) -> Self {
        Self {
            initial_range: range.clone(),
            free_ranges: smallvec::smallvec![range],
            alignment,
        }
    }

    pub const fn initial_range(&self) -> &Range<T> {
        &self.initial_range
    }

    pub fn grow_to(&mut self, new_end: T) {
        let initial_range_end = self.initial_range.end;
        if let Some(last_range) = self
            .free_ranges
            .last_mut()
            .filter(|last_range| last_range.end == initial_range_end)
        {
            last_range.end = new_end;
        } else {
            self.free_ranges.push(self.initial_range.end..new_end);
        }

        self.initial_range.end = new_end;
    }

    pub fn allocate_range(&mut self, pages: T) -> Result<Range<T>, RangeAllocationError<T>> {
        assert_ne!(pages + pages, pages);
        let length = pages * self.alignment;
        debug!("Allocating range with size {length:x?} ({pages:?} pages)");
        let mut best_fit: Option<(usize, Range<T>)> = None;

        // This is actually correct. With the trait bound as it is, we have
        // no way to summon a value of 0 directly, so we make one by subtracting
        // something from itself. Once the trait bound can be changed, this can
        // be fixed.
        #[allow(clippy::eq_op)]
        let mut fragmented_free_length = length - length;
        for (index, range) in self.free_ranges.iter().cloned().enumerate() {
            let range_length = range.end - range.start;
            fragmented_free_length += range_length;
            if range_length < length {
                continue;
            } else if range_length == length {
                // Found a perfect fit, so stop looking.
                best_fit = Some((index, range));
                break;
            }
            best_fit = Some(match best_fit {
                Some((best_index, best_range)) => {
                    // Find best fit for this allocation to reduce memory fragmentation.
                    if range_length < best_range.end - best_range.start {
                        (index, range)
                    } else {
                        (best_index, best_range.clone())
                    }
                }
                None => (index, range),
            });
        }
        match best_fit {
            Some((index, range)) => {
                if range.end - range.start == length {
                    self.free_ranges.remove(index);
                } else {
                    self.free_ranges[index].start += length;
                }

                debug!(
                    "Allocated range at {:x?}-{:x?}",
                    range.start,
                    range.start + length,
                );
                Ok(range.start..(range.start + length))
            }
            None => Err(RangeAllocationError {
                fragmented_free_length,
            }),
        }
    }

    pub fn free_range(&mut self, range: Range<T>) {
        assert!(range.start < range.end);
        // debug!("Freeing range {range:x?}");
        // Expand managed domain if necessary
        if range.start < self.initial_range.start {
            self.initial_range.start = range.start;
        }
        if range.end > self.initial_range.end {
            self.initial_range.end = range.end;
        }

        // Get insertion position.
        let i = self
            .free_ranges
            .iter()
            .position(|r| r.start > range.start)
            .unwrap_or(self.free_ranges.len());

        // Try merging with neighboring ranges in the free list.
        // Before: |left|-(range)-|right|
        if i > 0 && range.start == self.free_ranges[i - 1].end {
            // Merge with |left|.
            self.free_ranges[i - 1].end =
                if i < self.free_ranges.len() && range.end == self.free_ranges[i].start {
                    // Check for possible merge with |left| and |right|.
                    let right = self.free_ranges.remove(i);
                    right.end
                } else {
                    range.end
                };

            return;
        } else if i < self.free_ranges.len() && range.end == self.free_ranges[i].start {
            // Merge with |right|.
            self.free_ranges[i].start = if i > 0 && range.start == self.free_ranges[i - 1].end {
                // Check for possible merge with |left| and |right|.
                let left = self.free_ranges.remove(i - 1);
                left.start
            } else {
                range.start
            };

            return;
        }

        // Debug checks
        assert!(
            (i == 0 || self.free_ranges[i - 1].end < range.start)
                && (i >= self.free_ranges.len() || range.end < self.free_ranges[i].start)
        );

        self.free_ranges.insert(i, range);
    }

    /// Returns an iterator over allocated non-empty ranges
    pub fn allocated_ranges(&self) -> impl Iterator<Item = Range<T>> + '_ {
        let first = match self.free_ranges.first() {
            Some(Range { start, .. }) if *start > self.initial_range.start => {
                Some(self.initial_range.start..*start)
            }
            None => Some(self.initial_range.clone()),
            _ => None,
        };

        let last = match self.free_ranges.last() {
            Some(Range { end, .. }) if *end < self.initial_range.end => {
                Some(*end..self.initial_range.end)
            }
            _ => None,
        };

        let mid = self
            .free_ranges
            .iter()
            .zip(self.free_ranges.iter().skip(1))
            .map(|(ra, rb)| ra.end..rb.start);

        first.into_iter().chain(mid).chain(last)
    }

    pub fn reset(&mut self) {
        self.free_ranges.clear();
        self.free_ranges.push(self.initial_range.clone());
    }

    pub fn is_empty(&self) -> bool {
        self.free_ranges.len() == 1 && self.free_ranges[0] == self.initial_range
    }
}

impl<const N: usize, T: Copy + Sub<Output = T> + Sum> RangeAllocator<T, N> {
    pub fn total_available(&self) -> T {
        self.free_ranges
            .iter()
            .map(|range| range.end - range.start)
            .sum()
    }
}

/// Initialize a small region allocator on top of BootInfo-derived free_start.
///
/// This creates a simple 1 GiB virtual window starting at `layout.free_start`.
/// The allocator we create is a very small "virtual-only" bump allocator: it returns
/// virtual addresses (unmapped) that your subsystems can map later to physical frames.
///
/// You can change `DEFAULT_WINDOW_BYTES` to a smaller/larger value depending on needs.
fn find_initial_range<S: PageSize, T: Translate>(
    layout: &KernelVirtLayout,
    translator: &T,
) -> Range<u64> {
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
    start.as_u64()..end.as_u64()
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

pub fn init_region_allocator<const N: usize, S: PageSize>(
    page_tables: &impl FreeTables,
    layout: &KernelVirtLayout,
    translator: &impl Translate,
) -> RangeAllocator<u64, N> {
    let mut allocator =
        RangeAllocator::new(find_initial_range::<S, _>(layout, translator), S::SIZE);
    let range_end = allocator.initial_range.end; // End of initial allocation & kernel data
    for (_, start, end) in page_tables
        .free_l4_kernel_entries()
        .map(|region| {
            let idx = u64::from(region.index);
            let start = idx * region.alignment;
            let end = (idx + 1) * region.alignment;
            (idx, start, end)
        })
        .skip_while(|(_, start, _)| *start < range_end)
        .take(N)
    {
        // info!("Including range: {start:x}-{end:x}");
        allocator.free_range(start..end);
    }

    allocator
}

pub struct FreeOnDrop(pub Range<u64>);

impl Drop for FreeOnDrop {
    fn drop(&mut self) {
        debug!("Freeing on drop {:x?}", self.0);
        let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();

        lock.virt_region_allocator.free_range(self.0.clone());

        drop(lock);
    }
}
