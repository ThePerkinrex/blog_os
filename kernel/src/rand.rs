use spin::{Lazy, lock_api::Mutex};
use uuid::Uuid;
use x86_64::instructions::random::RdRand;

fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") low, out("edx") high);
    }
    ((high as u64) << 32) | (low as u64)
}

fn get_entropy_seed() -> u64 {
    rdtsc() ^ 0x9E3779B97F4A7C15
}

enum SystemRng {
    RdRand(RdRand),
    Other(Mutex<Xoroshiro128>),
}

impl SystemRng {
    fn new() -> Self {
        RdRand::new().map_or_else(
            || Self::Other(Mutex::new(Xoroshiro128::new(get_entropy_seed()))),
            Self::RdRand,
        )
    }

    fn next_u64(&self) -> u64 {
        match self {
            Self::RdRand(rd_rand) => loop {
                if let Some(res) = rd_rand.get_u64() {
                    break res;
                }
            },
            Self::Other(mutex) => mutex.lock().next_u64(),
        }
    }

    fn next_u128(&self) -> u128 {
        ((self.next_u64() as u128) << 64) | (self.next_u64() as u128)
    }
}

pub struct Xoroshiro128 {
    state: [u64; 2],
}

impl Xoroshiro128 {
    pub fn new(seed: u64) -> Self {
        let mut s = [seed, seed ^ 0x9E3779B97F4A7C15];
        for _ in 0..16 {
            s[0] = s[0].wrapping_add(s[1]);
        } // simple mix
        Self { state: s }
    }

    pub const fn next_u64(&mut self) -> u64 {
        let s0 = self.state[0];
        let mut s1 = self.state[1];
        let result = s0.wrapping_add(s1);

        s1 ^= s0;
        self.state[0] = s0.rotate_left(55) ^ s1 ^ (s1 << 14);
        self.state[1] = s1.rotate_left(36);

        result
    }
}

static RAND: Lazy<SystemRng> = Lazy::new(SystemRng::new);

pub fn uuid_v4() -> Uuid {
    Uuid::from_u128(RAND.next_u128())
}
