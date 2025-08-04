// cli/src/nonosctl/onion.rs — NØNOS Onion Routing Capsule Protocol
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Full x25519 mesh routing, MAC verification, relay registry, and zk capsule envelope
// A huge thanks to anyone.io for building the trustless network for Anyone.

use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};
use x25519_dalek::{EphemeralSecret, StaticSecret, PublicKey as X25519Pub};
use serde::{Serialize, Deserialize};
use rand::{rngs::OsRng, RngCore};
use chrono::{Utc, DateTime};
use base58::{FromBase58, ToBase58};
use blake3;
use std::collections::{HashSet, HashMap};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use lazy_static::lazy_static;

const NONCE_SIZE: usize = 12;
const MAX_HOPS: usize = 5;
const RELAY_REGISTRY: &str = "/etc/nonos/relays.json";

lazy_static! {
    static ref REPLAY_CACHE: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    static ref ROUTE_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OnionEnvelope {
    pub final_mac: Vec<u8>,
    pub layers: Vec<HopFrame>,
    pub capsule_type: String,
    pub created_at: String,
    pub origin_id: String,
    pub zk_identity: Option<String>,
    pub relay_route: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HopFrame {
    pub encrypted: Vec<u8>,
    pub ephemeral_pub: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ttl: u8,
    pub timestamp: String,
    pub hop_id: String,
    pub zk_proof: Option<String>,
    pub exit_code: Option<String>,
    pub mac_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayHop {
    pub pubkey: Vec<u8>,
    pub hop_id: String,
    pub ttl: u8,
    pub zk_hint: Option<String>,
}

pub fn wrap_v3(payload: &[u8], hops: &[RelayHop], capsule_type: &str, origin: &str, zk_identity: Option<String>) -> OnionEnvelope {
    let mut data = payload.to_vec();
    let mut layers = Vec::new();
    let mut relay_ids = Vec::new();

    for hop in hops.iter().rev() {
        let eph_secret = EphemeralSecret::new(OsRng);
        let eph_pub = X25519Pub::from(&eph_secret);
        let peer_pub = X25519Pub::from(<[u8; 32]>::try_from(hop.pubkey.clone()).unwrap());
        let shared_secret = eph_secret.diffie_hellman(&peer_pub);

        let mut nonce = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce);
        let aead_key = Key::from_slice(&blake3::hash(shared_secret.as_bytes()).as_bytes()[..32]);
        let cipher = Aes256Gcm::new(aead_key);

        let frame = HopFrame {
            encrypted: vec![],
            ephemeral_pub: eph_pub.as_bytes().to_vec(),
            nonce: nonce.to_vec(),
            ttl: hop.ttl,
            timestamp: Utc::now().to_rfc3339(),
            hop_id: hop.hop_id.clone(),
            zk_proof: hop.zk_hint.clone(),
            exit_code: None,
            mac_hint: Some(hex::encode(blake3::hash(&nonce).as_bytes())),
        };

        let serialized = bincode::serialize(&(frame.clone(), data)).unwrap();
        let encrypted = cipher.encrypt(Nonce::from_slice(&nonce), serialized.as_ref()).unwrap();

        let mut final_frame = frame;
        final_frame.encrypted = encrypted;
        data = bincode::serialize(&final_frame).unwrap();

        relay_ids.push(hop.hop_id.clone());
        layers.push(final_frame);
    }

    let mac = blake3::hash(&data);
    OnionEnvelope {
        final_mac: mac.as_bytes().to_vec(),
        layers: layers.into_iter().rev().collect(),
        capsule_type: capsule_type.into(),
        created_at: Utc::now().to_rfc3339(),
        origin_id: origin.into(),
        zk_identity,
        relay_route: relay_ids.into_iter().rev().collect(),
    }
}

pub fn unwrap_v3(envelope: &OnionEnvelope, privkey: &[u8]) -> Option<(Vec<u8>, Option<HopFrame>)> {
    let layer = envelope.layers.first()?;
    if is_replay(&layer.nonce) {
        return None;
    }
    cache_nonce(&layer.nonce);
    log_hop(&layer.hop_id);

    let sk = StaticSecret::from(<[u8; 32]>::try_from(privkey).ok()?);
    let peer_ephemeral = X25519Pub::from(<[u8; 32]>::try_from(layer.ephemeral_pub.clone()).ok()?);
    let shared_secret = sk.diffie_hellman(&peer_ephemeral);
    let aead_key = Key::from_slice(&blake3::hash(shared_secret.as_bytes()).as_bytes()[..32]);
    let cipher = Aes256Gcm::new(aead_key);

    let decrypted = cipher.decrypt(Nonce::from_slice(&layer.nonce), layer.encrypted.as_ref()).ok()?;
    let result: Result<(HopFrame, Vec<u8>), _> = bincode::deserialize(&decrypted);

    match result {
        Ok((next_frame, payload)) => Some((payload, Some(next_frame))),
        Err(_) => Some((decrypted, None)),
    }
}

pub fn verify_mac_chain(env: &OnionEnvelope) -> bool {
    if let Some(last) = env.layers.last() {
        let raw = bincode::serialize(last).ok()?;
        blake3::hash(&raw).as_bytes() == env.final_mac.as_slice()
    } else {
        false
    }
}

fn is_replay(nonce: &[u8]) -> bool {
    let key = hex::encode(nonce);
    REPLAY_CACHE.lock().unwrap().contains(&key)
}

fn cache_nonce(nonce: &[u8]) {
    let key = hex::encode(nonce);
    REPLAY_CACHE.lock().unwrap().insert(key);
}

fn log_hop(hop_id: &str) {
    ROUTE_LOG.lock().unwrap().push(hop_id.into());
}

pub fn relay_registry() -> Vec<RelayHop> {
    if !Path::new(RELAY_REGISTRY).exists() {
        return vec![];
    }
    let file = fs::read_to_string(RELAY_REGISTRY).unwrap_or_default();
    serde_json::from_str::<Vec<RelayHop>>(&file).unwrap_or_default()
}

pub fn print_envelope(env: &OnionEnvelope) {
    println!("\n=== Onion Capsule ===");
    println!("Type: {} | Created: {} | Origin: {}", env.capsule_type, env.created_at, env.origin_id);
    println!("ZK: {:?} | Route: {:?}", env.zk_identity, env.relay_route);
    println!("Layers: {}", env.layers.len());
    for (i, h) in env.layers.iter().enumerate() {
        println!("  Hop #{} -> {} | TTL {} | zk {:?} | exit {:?}", i + 1, h.hop_id, h.ttl, h.zk_proof, h.exit_code);
    }
}

pub fn is_final_hop(env: &OnionEnvelope) -> bool {
    env.layers.len() == 1
}

pub fn print_route_log() {
    println!("\n[route-log] {} hops forwarded:", ROUTE_LOG.lock().unwrap().len());
    for h in ROUTE_LOG.lock().unwrap().iter() {
        println!(" → {}", h);
    }
}

