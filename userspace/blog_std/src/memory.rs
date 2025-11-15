use talc::{OomHandler, Span, Talc, Talck};

use crate::{brk, lock::RawYieldingMutex, nop, println};

#[derive(Debug)]
struct GrowHeap {
    span: Option<(Span, usize)>,
}

const MIN_HEAP: usize = 0x80000; // 128 KB

impl OomHandler for GrowHeap {
    fn handle_oom(talc: &mut Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
        let requested = layout.pad_to_align();
        let requested_size = requested.size().max(MIN_HEAP);
        nop(requested_size as u64);
        if let Some((old_span, original_brk)) = talc.oom_handler.span {
            let grown = brk(requested_size.try_into().map_err(|_| ())?);
            let span = Span::new(original_brk as *mut u8, grown);
            let span = unsafe { talc.extend(old_span, span) };

            talc.oom_handler.span = Some((span, original_brk));
        } else {
            let original_brk = brk(0);
            let grown = brk(requested_size.try_into().map_err(|_| ())?);
            let span = Span::new(original_brk, grown);

            let span = unsafe { talc.claim(span) }?;

            talc.oom_handler.span = Some((span, original_brk as usize));
        }
        Ok(())
    }
}

#[global_allocator]
static ALLOCATOR: Talck<RawYieldingMutex, GrowHeap> = Talc::new(GrowHeap { span: None }).lock();
