use spin::Once;

static SESSION_KEY: Once<[u8; 32]> = Once::new();

pub fn derive_session(hash: &[u8; 32]) {
    let key = blake3::keyed_hash(b"NONOS_SESSION", hash).into();
    SESSION_KEY.call_once(|| key);
}

pub fn seal(data: &[u8]) -> alloc::vec::Vec<u8> {
    let key = SESSION_KEY.wait();
    // XChaCha20-Poly1305 seal (implement or call tinycrypt)
    pseudo_encrypt(data, key)
}
