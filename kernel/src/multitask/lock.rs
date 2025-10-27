use lock_api::{GuardSend, RawMutex};

use crate::{multitask::{TaskId, get_current_task_id}, println};

pub struct ReentrantRawMutex {
    inner: spin::Mutex<ReentrantInner>,
}

struct ReentrantInner {
    owner: Option<TaskId>,
    depth: usize,
}

impl ReentrantRawMutex {
    pub const fn new() -> Self {
        Self {
            inner: spin::Mutex::new(ReentrantInner {
                owner: None,
                depth: 0,
            }),
        }
    }

    

    // /// Internal: give mutable access. Unsafe because caller must hold lock.
    // unsafe fn data(&self) -> &mut T {
    //     // NOTE: we take inner lock briefly to read/validate that current thread owns it.
    //     let guard = self.inner.lock();
    //     let me = get_current_task_id();
    //     debug_assert!(guard.owner == Some(me), "must hold lock to access data");
    //     unsafe {&mut *guard.data.get()}
    // }

    /// For advanced users: query recursion depth (0 if unlocked).
    pub fn recursion_depth(&self) -> usize {
        let guard = self.inner.lock();
        guard.depth
    }
}

impl Default for ReentrantRawMutex {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl RawMutex for ReentrantRawMutex {
	const INIT: Self = Self::new();

	type GuardMarker = GuardSend;

	/// Acquire the reentrant lock. Spins if another thread holds it.
    fn lock(&self) {
		println!("[INFO][LOCK] Locking reentrant_lock");
        let me = get_current_task_id();
		println!("[INFO][LOCK] For id {me:?}");

        loop {
            {
                let mut guard = self.inner.lock();
                match guard.owner {
                    None => {
                        // nobody holds it -> take ownership
                        guard.owner = Some(me);
                        guard.depth = 1;
                        // return guard (we drop the spin::Mutex guard but keep logical ownership)
                        return;
                    }
                    Some(owner) if owner == me => {
                        // Reentrant lock by same thread: increase depth
                        guard.depth = guard.depth.checked_add(1).expect("reentrant depth overflow");
                        return;
                    }
                    _ => {
                        // someone else holds it, drop inner guard and spin
                    }
                }
            }
            // Optional: call yield or schedule; fallback to spin loop.
            x86_64::instructions::hlt();
        }
    }

    /// Try to lock without blocking. Returns None if a different thread owns it.
    fn try_lock(&self) -> bool {
        let me = get_current_task_id();
        let Some(mut guard) = self.inner.try_lock() else {
			return false;
		};
        match guard.owner {
            None => {
                guard.owner = Some(me);
                guard.depth = 1;
                true
            }
            Some(owner) if owner == me => {
                guard.depth = guard.depth.checked_add(1).expect("reentrant depth overflow");
                true
            }
            _ => false,
        }
    }

	unsafe fn unlock(&self) {
		println!("[INFO][LOCK] Unlocking reentrant_lock");
        let me = get_current_task_id();
		println!("[INFO][LOCK] For id {me:?}");
        let mut guard = self.inner.lock();
        // must be owner
        debug_assert!(guard.owner == Some(me), "ReentrantGuard dropped by non-owner");
        // reduce depth and release when 0
        guard.depth -= 1;
        if guard.depth == 0 {
            guard.owner = None;
        }
		println!("[INFO][LOCK] Unlocked depth: {}", guard.depth);
	}
}

// pub struct ReentrantGuard<'a, T> {
//     rm: &'a ReentrantMutex<T>,
// }

// impl<'a, T> Deref for ReentrantGuard<'a, T> {
//     type Target = T;
//     fn deref(&self) -> &T {
//         // SAFETY: we ensure caller holds logical ownership; safe to get ref
//         unsafe { &*self.rm.inner.lock().data.get() }
//     }
// }
// impl<'a, T> DerefMut for ReentrantGuard<'a, T> {
//     fn deref_mut(&mut self) -> &mut T {
//         // SAFETY: same as above
//         unsafe { &mut *self.rm.inner.lock().data.get() }
//     }
// }

// impl<'a, T> Drop for ReentrantGuard<'a, T> {
//     fn drop(&mut self) {
//         let me = get_current_task_id();
//         let mut guard = self.rm.inner.lock();
//         // must be owner
//         debug_assert!(guard.owner == Some(me), "ReentrantGuard dropped by non-owner");
//         // reduce depth and release when 0
//         guard.depth -= 1;
//         if guard.depth == 0 {
//             guard.owner = None;
//         }
//         // spin::Mutex guard is dropped here (unlocked)
//     }
// }

pub type ReentrantMutex<T> = lock_api::Mutex<ReentrantRawMutex, T>;