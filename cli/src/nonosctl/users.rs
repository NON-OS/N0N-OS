// cli/src/nonosctl/users.rs — NØN-OS Identity Engine (Advanced Cryptographic Auth)

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use sha2::{Sha256, Digest};

const USER_DB: &str = "/var/nonos/auth/users.json";
const SESSION_FILE: &str = "/var/nonos/auth/sessions.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub username: String,
    pub public_key: String,
    pub joined: String,
    pub zk_enabled: bool,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub username: String,
    pub session_token: String,
    pub issued_at: String,
    pub valid: bool,
}

pub fn add_user(username: &str) {
    let mut users = load_users();
    if users.contains_key(username) {
        println!("[auth] user '{}' already exists.", username);
        return;
    }

    let key = generate_pubkey(username);
    let user = User {
        username: username.to_string(),
        public_key: key,
        joined: Utc::now().to_rfc3339(),
        zk_enabled: false,
        flags: vec![],
    };

    users.insert(username.to_string(), user);
    save_users(&users);
    println!("[auth] user '{}' added.", username);
}

pub fn remove_user(username: &str) {
    let mut users = load_users();
    if users.remove(username).is_some() {
        save_users(&users);
        println!("[auth] user '{}' removed.", username);
    } else {
        println!("[auth] user '{}' not found.", username);
    }
}

pub fn list_users() {
    let users = load_users();
    if users.is_empty() {
        println!("[auth] no registered users.");
    } else {
        for (name, user) in users {
            println!("[auth] {} | zk={} | flags={:?}", name, user.zk_enabled, user.flags);
        }
    }
}

pub fn enable_zk(username: &str) {
    let mut users = load_users();
    if let Some(user) = users.get_mut(username) {
        user.zk_enabled = true;
        println!("[auth] zk-login enabled for '{}'.", username);
        save_users(&users);
    } else {
        println!("[auth] user '{}' not found.", username);
    }
}

pub fn user_info(username: &str) {
    let users = load_users();
    if let Some(user) = users.get(username) {
        println!("[auth] '{}':", user.username);
        println!(" - joined: {}", user.joined);
        println!(" - zk_enabled: {}", user.zk_enabled);
        println!(" - public_key: {}", user.public_key);
        println!(" - flags: {:?}", user.flags);
    } else {
        println!("[auth] user '{}' not found.", username);
    }
}

pub fn login_user(username: &str) {
    let users = load_users();
    if users.contains_key(username) {
        let token = generate_token(username);
        let mut sessions = load_sessions();
        sessions.insert(username.to_string(), Session {
            username: username.to_string(),
            session_token: token.clone(),
            issued_at: Utc::now().to_rfc3339(),
            valid: true,
        });
        save_sessions(&sessions);
        println!("[auth] user '{}' logged in with session token: {}", username, token);
    } else {
        println!("[auth] login failed: user '{}' not found.", username);
    }
}

pub fn validate_session(username: &str, token: &str) {
    let sessions = load_sessions();
    if let Some(sess) = sessions.get(username) {
        if sess.session_token == token && sess.valid {
            println!("[auth] session token valid for '{}'.", username);
        } else {
            println!("[auth] invalid or expired session for '{}'.", username);
        }
    } else {
        println!("[auth] no session found for '{}'.", username);
    }
}

fn generate_pubkey(seed: &str) -> String {
    let rand_part: String = rand::thread_rng().sample_iter(&Alphanumeric).take(16).map(char::from).collect();
    let combined = format!("{}-{}", seed, rand_part);
    let mut hasher = Sha256::new();
    hasher.update(combined.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_token(seed: &str) -> String {
    let raw: String = format!("{}:{}:{}", seed, Utc::now(), rand::thread_rng().gen::<u64>());
    let mut hasher = Sha256::new();
    hasher.update(raw);
    format!("{:x}", hasher.finalize())
}

fn load_users() -> HashMap<String, User> {
    if let Ok(json) = fs::read_to_string(USER_DB) {
        serde_json::from_str(&json).unwrap_or_default()
    } else {
        HashMap::new()
    }
}

fn save_users(users: &HashMap<String, User>) {
    if let Ok(json) = serde_json::to_string_pretty(users) {
        let _ = fs::write(USER_DB, json);
    }
}

fn load_sessions() -> HashMap<String, Session> {
    if let Ok(json) = fs::read_to_string(SESSION_FILE) {
        serde_json::from_str(&json).unwrap_or_default()
    } else {
        HashMap::new()
    }
}

fn save_sessions(sessions: &HashMap<String, Session>) {
    if let Ok(json) = serde_json::to_string_pretty(sessions) {
        let _ = fs::write(SESSION_FILE, json);
    }
}
