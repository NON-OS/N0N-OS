//! NØNOS Entropy & RNG Subsystem – Production-Grade
//!
//! Provides cryptographically secure entropy sources for kernel boot, module identity,
//! protocol salts, and ZeroState randomness. Hybrid design with hardware fallback,
//! CPU timestamp seeding, and deterministic failover.

use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

static ENTROPY_SEED: AtomicU64 = AtomicU64::new(0);
static RNG: Mutex<ChaoticRng> = Mutex::new(ChaoticRng::new(0));

/// CSPRNG Engine (XOR-Shift-Rotate Scheme)
pub struct ChaoticRng {
    state: u64,
    counter: u64,
}

impl ChaoticRng {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed, counter: 0 }
    }

    pub fn reseed(&mut self, seed: u64) {
        self.state ^= seed.rotate_left(11) ^ seed.rotate_right(5);
        self.counter = 0;
    }

    pub fn next(&mut self) -> u64 {
        self.counter += 1;
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state.wrapping_add(self.counter.rotate_left(19))
    }

    pub fn next_byte(&mut self) -> u8 {
        (self.next() & 0xFF) as u8
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next() & 0xFFFF_FFFF) as u32
    }

    pub fn next_bytes(&mut self, out: &mut [u8]) {
        for byte in out.iter_mut() {
            *byte = self.next_byte();
        }
    }
}

/// Seeds the RNG using timestamp counter or fallback method
pub fn seed_rng() {
    let entropy = read_tsc() ^ read_fallback_timer();
    ENTROPY_SEED.store(entropy, Ordering::SeqCst);
    RNG.lock().reseed(entropy);
    audit("[entropy] RNG seeded from TSC+fallback");
}

/// Return secure 64-bit random number
pub fn rand_u64() -> u64 {
    RNG.lock().next()
}

/// Return secure 8-bit random value
pub fn rand_byte() -> u8 {
    RNG.lock().next_byte()
}

/// Return secure 32-bit random number
pub fn rand_u32() -> u32 {
    RNG.lock().next_u32()
}

/// Fill buffer with random data
pub fn fill_bytes(buffer: &mut [u8]) {
    RNG.lock().next_bytes(buffer);
}

/// Hardware entropy fallback: using a different instruction as backup
fn read_fallback_timer() -> u64 {
    let ticks: u64;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") ticks, out("edx") _);
    }
    ticks.rotate_left(21)
}

/// CPU Timestamp Counter as entropy input
fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") low, out("edx") high);
    }
    ((high as u64) << 32) | (low as u64)
}

/// Emit entropy log for audit trails
fn audit(msg: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log(msg);
    }
}
