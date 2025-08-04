// cli/src/main.rs — NØN-OS Foundational CLI Entrypoint
// Written with hearth and respect of peoples value
// As privacy is a fundamental human right
// eK <3

use clap::{Parser, Subcommand, Args};
use std::fs;
use std::path::Path;
use serde_json::json;

mod nonosctl;
use nonosctl::{users, logging, capsule, services, capsule_net};

const CONFIG_PATH: &str = "/etc/nonos/config.toml";

#[derive(Parser)]
#[command(
    name = "nonosctl",
    version = "0.3.0",
    author = "NØNOS core@dev",
    about = "nonosctl — Sovereign Runtime Interface for NØNOS",
    long_about = "nonosctl is the capsule-native system interface for sovereign runtime control, identity, audit, and service orchestration."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable JSON output
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    Capsule {
        #[command(subcommand)]
        action: CapsuleAction,
    },
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    Mesh {
        #[command(subcommand)]
        action: MeshAction,
    },
    Log {
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    FlushLog,
    ExportLog {
        path: String,
    },
    Env,
    Stats,
    Init,
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Dev {
        #[command(subcommand)]
        action: DevAction,
    },
    Sysinfo,
}

#[derive(Subcommand)]
enum UserAction {
    Add { username: String },
    Remove { username: String },
    List,
    Info { username: String },
    EnableZk { username: String },
    Login { username: String },
    Session { username: String, token: String },
}

#[derive(Subcommand)]
enum CapsuleAction {
    Deploy { name: String, path: String },
    Run { name: String },
    Verify { name: String },
    Logs { name: String },
    List,
    Info { name: String },
    Delete { name: String },
}

#[derive(Subcommand)]
enum ServiceAction {
    Start { name: String },
    Stop { name: String },
    Restart { name: String },
    Status { name: String },
    Logs { name: String },
}

#[derive(Subcommand)]
enum MeshAction {
    Start,
}

#[derive(Subcommand)]
enum ConfigAction {
    View,
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum DevAction {
    MockUser { name: String },
    WipeAll,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::User { action } => match action {
            UserAction::Add { username } => users::add_user(&username),
            UserAction::Remove { username } => users::remove_user(&username),
            UserAction::List => users::list_users(),
            UserAction::Info { username } => users::user_info(&username),
            UserAction::EnableZk { username } => users::enable_zk(&username),
            UserAction::Login { username } => users::login_user(&username),
            UserAction::Session { username, token } => users::validate_session(&username, &token),
        },

        Commands::Capsule { action } => match action {
            CapsuleAction::Deploy { name, path } => capsule::deploy_capsule(&name, &path),
            CapsuleAction::Run { name } => capsule::run_capsule(&name),
            CapsuleAction::Verify { name } => capsule::verify_capsule(&name),
            CapsuleAction::Logs { name } => capsule::capsule_logs(&name),
            CapsuleAction::List => capsule::list_capsules(cli.json),
            CapsuleAction::Info { name } => capsule::capsule_info(&name, cli.json),
            CapsuleAction::Delete { name } => capsule::delete_capsule(&name),
        },

        Commands::Service { action } => match action {
            ServiceAction::Start { name } => services::start_service(&name),
            ServiceAction::Stop { name } => services::stop_service(&name),
            ServiceAction::Restart { name } => services::restart_service(&name),
            ServiceAction::Status { name } => services::service_status(&name),
            ServiceAction::Logs { name } => services::service_logs(&name),
        },

        Commands::Mesh { action } => match action {
            MeshAction::Start => {
                let dummy_priv = include_bytes!("../../keys/dev.key");
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    capsule_net::start_capsule_mesh(dummy_priv, "core.peer".into()).await;
                });
            }
        },

        Commands::Log { limit } => logging::view_audit_log(limit),
        Commands::FlushLog => logging::flush_audit_log(),
        Commands::ExportLog { path } => logging::export_audit_log(&path),
        Commands::Stats => logging::audit_stats(),

        Commands::Env => {
            let env = json!({
                "hostname": "nonos-devnet.local",
                "mode": "SAFE",
                "zk": true,
                "arch": "capsule-native"
            });
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&env).unwrap());
            } else {
                println!("[env] hostname: {}", env["hostname"]);
                println!("[env] mode: {}", env["mode"]);
                println!("[env] zk: {}", env["zk"]);
                println!("[env] arch: {}", env["arch"]);
            }
        },

        Commands::Init => {
            fs::create_dir_all("/var/nonos/logs").ok();
            fs::create_dir_all("/var/nonos/auth").ok();
            fs::create_dir_all("/etc/nonos").ok();
            fs::create_dir_all("/var/nonos/capsules").ok();
            fs::write("/var/nonos/auth/users.json", b"{}").ok();
            fs::write("/etc/nonos/config.toml", b"default_mode = 'SAFE'\nzk_enabled = true\n").ok();
            fs::write("/var/nonos/capsules/index.json", b"{}").ok();
            println!("[init] system folders and files initialized.");
        },

        Commands::Config { action } => match action {
            ConfigAction::View => {
                if let Ok(cfg) = fs::read_to_string(CONFIG_PATH) {
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
                    } else {
                        println!("{}", cfg);
                    }
                } else {
                    println!("[config] config file not found.");
                }
            },
            ConfigAction::Set { key, value } => {
                let mut current = fs::read_to_string(CONFIG_PATH).unwrap_or_default();
                let new_line = format!("{} = '{}'\n", key, value);
                current.push_str(&new_line);
                fs::write(CONFIG_PATH, current).ok();
                println!("[config] set {} = '{}'", key, value);
            }
        },

        Commands::Dev { action } => match action {
            DevAction::MockUser { name } => {
                users::add_user(&name);
                users::enable_zk(&name);
                println!("[dev] mock user '{}' created with zk-login", name);
            },
            DevAction::WipeAll => {
                fs::remove_file("/var/nonos/auth/users.json").ok();
                fs::remove_file("/var/nonos/logs/audit.log").ok();
                fs::remove_file("/var/nonos/capsules/index.json").ok();
                println!("[dev] all user, audit, and capsule data wiped.");
            }
        },

        Commands::Sysinfo => {
            let uptime = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
            let mem = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
            println!("[sysinfo] uptime: {}", uptime.lines().next().unwrap_or("n/a"));
            println!("[sysinfo] memory:\n{}", mem.lines().take(5).collect::<Vec<_>>().join("\n"));
        }
    }
}

