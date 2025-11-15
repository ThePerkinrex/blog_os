use core::sync::atomic::{AtomicBool, Ordering};

use lock_api::{GuardSend, RawMutex};

use crate::yield_syscall;

// 1. Define our raw lock type
pub struct RawYieldingMutex(AtomicBool);

// 2. Implement RawMutex for this type
unsafe impl RawMutex for RawYieldingMutex {
    const INIT: Self = Self(AtomicBool::new(false));

    // A spinlock guard can be sent to another thread and unlocked there
    type GuardMarker = GuardSend;

    fn lock(&self) {
        // Note: This isn't the best way of implementing a spinlock, but it
        // suffices for the sake of this example.
        while !self.try_lock() {
            yield_syscall();
        }
    }

    fn try_lock(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }
}

// 3. Export the wrappers. This are the types that your users will actually use.
pub type YieldingMutex<T> = lock_api::Mutex<RawYieldingMutex, T>;
pub type YieldingMutexGuard<'a, T> = lock_api::MutexGuard<'a, RawYieldingMutex, T>;
