// cli/src/nonosctl/capsule_net.rs — Capsule Mesh 
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Decentralized zk-auth capsule router with full mesh acknowledgment

use crate::capsule::{CapsulePayload, create_capsule, unwrap_capsule, verify_capsule_sig};
use crate::onion::{RelayHop, OnionEnvelope};
use crate::capsule_runtime::store_capsule;
use crate::logging::log_event;
use libp2p::{floodsub::{Floodsub, Topic}, identity, PeerId, Swarm, Multiaddr, NetworkBehaviour, mdns::{Mdns, MdnsConfig, MdnsEvent}};
use libp2p::core::upgrade;
use libp2p::tcp::TokioTcpConfig;
use libp2p::noise::{NoiseConfig, X25519Spec, Keypair as NoiseKeypair, AuthenticKeypair, NoiseAuthenticated};
use libp2p::yamux::YamuxConfig;
use libp2p::swarm::SwarmBuilder;
use libp2p::Transport;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;
use tokio::sync::mpsc;
use std::fs;
use std::path::Path;
use std::time::Duration;

const CAPSULE_BROADCAST_PATH: &str = "/var/nonos/capsules/queue";
const MESH_TOPIC: &str = "nonos.capsule.mesh";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapsuleTransfer {
    pub capsule_id: String,
    pub capsule: Vec<u8>,
    pub origin: String,
    pub zk_required: bool,
    pub timestamp: String,
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NetEvent")]
pub struct MeshBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,
}

#[derive(Debug)]
pub enum NetEvent {
    Floodsub(libp2p::floodsub::FloodsubEvent),
    Mdns(MdnsEvent),
}

impl From<libp2p::floodsub::FloodsubEvent> for NetEvent {
    fn from(event: libp2p::floodsub::FloodsubEvent) -> Self {
        NetEvent::Floodsub(event)
    }
}

impl From<MdnsEvent> for NetEvent {
    fn from(event: MdnsEvent) -> Self {
        NetEvent::Mdns(event)
    }
}

pub async fn start_capsule_mesh(privkey: &[u8], peer_tag: String) {
    let local_key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(local_key.public());
    println!("[capsule-mesh] Local Peer ID: {}", peer_id);

    let transport = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(NoiseKeypair::<X25519Spec>::new().into_authentic(&local_key).unwrap()))
        .multiplex(YamuxConfig::default())
        .boxed();

    let mut floodsub = Floodsub::new(peer_id);
    let topic = Topic::new(MESH_TOPIC);
    floodsub.subscribe(topic.clone());

    let mdns = Mdns::new(MdnsConfig::default()).await.unwrap();
    let behaviour = MeshBehaviour { floodsub, mdns };
    let mut swarm = SwarmBuilder::new(transport, behaviour, peer_id).executor(Box::new(|fut| { tokio::spawn(fut); })).build();

    let (tx, mut rx) = mpsc::channel::<CapsulePayload>(16);
    let mut seen_ids: HashSet<String> = HashSet::new();

    tokio::spawn(async move {
        while let Some(capsule) = rx.recv().await {
            if seen_ids.contains(&capsule.capsule_id) { continue; }
            seen_ids.insert(capsule.capsule_id.clone());
            let capsule_bytes = bincode::serialize(&capsule).unwrap();
            let transfer = CapsuleTransfer {
                capsule_id: capsule.capsule_id.clone(),
                capsule: capsule_bytes,
                origin: capsule.origin.clone(),
                zk_required: capsule.zk_auth_context.is_some(),
                timestamp: capsule.timestamp.clone(),
            };
            let data = bincode::serialize(&transfer).unwrap();
            swarm.behaviour_mut().floodsub.publish(topic.clone(), data);
            println!("[capsule-mesh] forwarded capsule '{}'.", capsule.capsule_id);
        }
    });

    loop {
        match swarm.select_next_some().await {
            NetEvent::Floodsub(libp2p::floodsub::FloodsubEvent::Message(msg)) => {
                if let Ok(transfer): Result<CapsuleTransfer, _> = bincode::deserialize(&msg.data) {
                    if seen_ids.contains(&transfer.capsule_id) { continue; }
                    seen_ids.insert(transfer.capsule_id.clone());

                    if let Ok(capsule): Result<CapsulePayload, _> = bincode::deserialize(&transfer.capsule) {
                        if transfer.zk_required && capsule.zk_auth_context.is_none() {
                            println!("[capsule-mesh] rejected '{}' (ZK required)", capsule.capsule_id);
                            continue;
                        }
                        if !verify_capsule_sig(&capsule) {
                            println!("[capsule-mesh] rejected '{}' (invalid signature)", capsule.capsule_id);
                            continue;
                        }
                        store_capsule(&capsule.capsule_id, &capsule).ok();
                        println!("[capsule-mesh] received '{}'.", capsule.capsule_id);
                        log_event("capsule-mesh", &capsule.capsule_id, "received", &peer_tag, "verified capsule received");
                        let _ = tx.send(capsule).await;
                    }
                }
            },
            NetEvent::Mdns(MdnsEvent::Discovered(peers)) => {
                for (peer, _) in peers {
                    swarm.behaviour_mut().floodsub.add_node_to_partial_view(peer);
                }
            },
            _ => {}
        }
    }
}

pub fn enqueue_for_mesh(capsule: &CapsulePayload) {
    fs::create_dir_all(CAPSULE_BROADCAST_PATH).ok();
    let path = format!("{}/{}.bin", CAPSULE_BROADCAST_PATH, capsule.capsule_id);
    let _ = fs::write(path, bincode::serialize(capsule).unwrap());
}

pub fn flush_mesh_queue(tx: &mpsc::Sender<CapsulePayload>) {
    if let Ok(entries) = fs::read_dir(CAPSULE_BROADCAST_PATH) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(data) = fs::read(&path) {
                    if let Ok(capsule): Result<CapsulePayload, _> = bincode::deserialize(&data) {
                        let _ = tx.blocking_send(capsule);
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }
    }
}

