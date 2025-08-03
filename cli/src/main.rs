// cli/src/main.rs — nonosctl extended (deploy, user, logs)

use clap::{Parser, Subcommand, Args};
use std::process::{self, Command};
use std::time::Instant;
use std::thread::sleep;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "nonos")]
#[command(version = "0.1.0")]
#[command(about = "NØN-OS CLI — Trustless Terminal OS Preview", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Boot {
        #[arg(short, long)]
        verbose: bool,
    },
    Verify {
        #[arg(short, long)]
        path: Option<String>,
    },
    Status,
    Help,
    Logs,
    RunTest {
        #[arg(short, long)]
        capsule_id: Option<String>,
    },
    Launch {
        #[arg(short, long)]
        program: String,
    },
    NonosCtl(NonosCtlCommands),
    Exit,
}

#[derive(Subcommand, Debug)]
enum NonosCtlCommands {
    List,
    Start {
        #[arg(short, long)]
        service: String,
    },
    Status {
        #[arg(short, long)]
        service: String,
    },
    Deploy {
        #[arg(short, long)]
        capsule: String,
    },
    User {
        #[arg(short, long)]
        add: Option<String>,
    },
    Logs {
        #[arg(short, long)]
        service: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Boot { verbose } => {
            println!("\n🔐 Booting NØN-OS capsule...\n");
            let start = Instant::now();
            simulate_loading("Loading entropy", verbose);
            simulate_loading("Verifying zero-knowledge proofs", verbose);
            simulate_loading("Launching kernel runtime", verbose);
            let elapsed = start.elapsed();
            println!("\n✅ Boot complete. System ready. [{}ms]\n", elapsed.as_millis());
        }
        Commands::Verify { path } => {
            println!("\n🔎 Starting capsule verification...");
            if let Some(p) = path {
                println!("Using mock capsule path: {}", p);
            } else {
                println!("No path provided. Running default mock validation...");
            }
            simulate_loading("Reading capsule metadata", true);
            simulate_loading("Checking cryptographic signatures", true);
            println!("✅ Capsule verification successful.\n");
        }
        Commands::Status => {
            println!("\n🧠 System Status:");
            println!(" - Runtime: In-memory");
            println!(" - Trust Model: Zero-Trust");
            println!(" - Capsule Engine: Operational");
            println!(" - ZK Verifier: Active\n");
        }
        Commands::Help => {
            println!("\n📖 Available Commands:");
            println!("  boot         => Launches the OS capsule and verifies ZK proofs");
            println!("  verify       => Validates a capsule file (simulated)");
            println!("  status       => Shows system runtime state");
            println!("  help         => Lists all commands");
            println!("  logs         => Displays recent capsule/system logs");
            println!("  runtest      => Executes a test capsule by ID");
            println!("  launch       => Launches a user-defined program (like firefox)");
            println!("  nonosctl     => Manage system services and daemons");
            println!("  exit         => Exits the CLI cleanly\n");
        }
        Commands::Logs => {
            println!("\n📜 Recent Capsule Logs:");
            println!("[INFO] Capsule #abc123 executed in 142ms");
            println!("[INFO] Proof verified successfully");
            println!("[WARN] Entropy module lag: +8ms");
            println!("[INFO] Capsule #xyz789 loaded from cache\n");
        }
        Commands::RunTest { capsule_id } => {
            let id = capsule_id.unwrap_or_else(|| "sandbox-test-001".to_string());
            println!("\n🧪 Running test capsule: {}", id);
            simulate_loading("Initializing sandbox", true);
            simulate_loading("Executing logic block", true);
            simulate_loading("Verifying internal assertions", true);
            println!("✅ Test capsule {} passed successfully.\n", id);
        }
        Commands::Launch { program } => {
            println!("\n🚀 Attempting to launch external program: {}", program);
            match Command::new(program).spawn() {
                Ok(_) => println!("✅ Program launched successfully.\n"),
                Err(e) => println!("❌ Failed to launch program: {}\n", e),
            }
        }
        Commands::NonosCtl(subcmd) => match subcmd {
            NonosCtlCommands::List => {
                println!("\n🛠️  Active Services:");
                println!(" - zkdaemon [running]");
                println!(" - capsule-cache [idle]");
                println!(" - net-core [running]");
            }
            NonosCtlCommands::Start { service } => {
                println!("\n⚙️  Starting service '{}':", service);
                simulate_loading("Initializing daemon", true);
                println!("✅ Service '{}' started successfully.\n", service);
            }
            NonosCtlCommands::Status { service } => {
                println!("\n🔍 Status of '{}':", service);
                simulate_loading("Querying runtime state", true);
                println!("[{}]: Operational\n", service);
            }
            NonosCtlCommands::Deploy { capsule } => {
                println!("\n🚢 Deploying capsule '{}':", capsule);
                simulate_loading("Uploading to network", true);
                simulate_loading("Broadcasting proof metadata", true);
                println!("✅ Capsule '{}' deployed successfully.\n", capsule);
            }
            NonosCtlCommands::User { add } => {
                if let Some(user) = add {
                    println!("\n👤 Adding user '{}':", user);
                    simulate_loading("Generating keypair", true);
                    simulate_loading("Writing user to access list", true);
                    println!("✅ User '{}' added to system.\n", user);
                } else {
                    println!("⚠️  No user specified. Use --add <username>\n");
                }
            }
            NonosCtlCommands::Logs { service } => {
                println!("\n🧾 Logs for '{}':", service);
                println!("[{}] [INFO] Service initialized", service);
                println!("[{}] [INFO] Polling active capsule queue", service);
                println!("[{}] [WARN] Missed sync cycle — retrying", service);
                println!();
            }
        },
        Commands::Exit => {
            println!("👋 Exiting NØN-OS CLI. Goodbye.");
            process::exit(0);
        }
    }
}

fn simulate_loading(task: &str, verbose: bool) {
    print!("{:<40}", format!("{}...", task));
    use std::io::{self, Write};
    io::stdout().flush().unwrap();

    sleep(Duration::from_millis(600));
    if verbose {
        print!(" [OK]");
    }
    println!();
}

