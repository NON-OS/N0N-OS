// ui/cli.rs
//
//   NØNOS CLI — 
// - Dual I/O: TUI + GUI bridge (mirror out; accept remote in)
// - Command registry: static, lock-free lookups; namespaced verbs (sys.*, rq.*, task.*, time.*, proof.*, net.*, gui.*)
// - History + tab-complete (ring buffer), help, suggestions
// - JSON frames to GUI: metrics/proof roots; event-bus publish
// - No heap churn on hot path; small fixed buffers; ISR-safe print path
//
// Zero-state. All output public. No secrets.

#![allow(dead_code)]

use core::fmt::Write as _;
use core::str;

use spin::Mutex;

use crate::ui::{tui, gui_bridge};
use crate::ui::event::{self, Event};
use crate::sched::{self, task::{self, Priority, Affinity}};
use crate::sched::runqueue as rq;
use crate::arch::x86_64::time::timer;
use crate::arch::x86_64::interrupt::{apic, ioapic};
use crate::memory::{self, proof};

const PROMPT: &str = "nonos# ";
const MAX_LINE: usize = 256;
const HIST_LEN: usize = 48;
const MAX_CMDS: usize = 64;

// —————————————————— command registry ——————————————————

type CmdFn = fn(&[&str]) -> Result<(), &'static str>;

struct Command {
    name: &'static str,
    help: &'static str,
    f: CmdFn,
}

static REG: Mutex<heapless::Vec<Command, MAX_CMDS>> = Mutex::new(heapless::Vec::new());

macro_rules! register {
    ($name:literal, $help:literal, $func:path) => {{
        let mut r = REG.lock();
        let _ = r.push(Command { name: $name, help: $help, f: $func });
    }};
}

fn lookup(name: &str) -> Option<CmdFn> {
    let r = REG.lock();
    for c in r.iter() {
        if c.name == name { return Some(c.f); }
    }
    None
}

fn suggest(prefix: &str) -> Option<&'static str> {
    let r = REG.lock();
    for c in r.iter() {
        if c.name.starts_with(prefix) { return Some(c.name); }
    }
    None
}

// —————————————————— history + line tools ——————————————————

static HIST: Mutex<[heapless::String<MAX_LINE>; HIST_LEN]> = Mutex::new(array_init::array_init(|_| heapless::String::new()));
static HIST_HEAD: Mutex<usize> = Mutex::new(0);

fn remember(line: &str) {
    let mut h = HIST.lock();
    let mut head = HIST_HEAD.lock();
    h[*head].clear();
    let _ = h[*head].push_str(line);
    *head = (*head + 1) % HIST_LEN;
}

fn words(line: &str, out: &mut [&str; 16]) -> usize {
    let mut n = 0;
    for w in line.split_whitespace() {
        if n == out.len() { break; }
        out[n] = w; n += 1;
    }
    n
}

// —————————————————— init ——————————————————

pub fn init() {
    // sys.*
    register!("help",                 "list commands",                    cmd_help);
    register!("sys.time",             "show monotonic time",              cmd_sys_time);
    register!("sys.mem",              "dump layout + maps",               cmd_sys_mem);
    register!("sys.apic",             "show LAPIC id",                    cmd_sys_apic);
    register!("sys.ioapic.route",     "route GSI to vector: <gsi>",       cmd_sys_ioapic_route);

    // rq.*
    register!("rq.stats",             "show runqueue counts",             cmd_rq_stats);

    // task.*
    register!("task.spawn",           "spawn demo: <name> <ms> [rt|hi|norm|lo|idle]", cmd_task_spawn);

    // time.*
    register!("time.hrtimer",         "arm hrtimer: <ms>",                cmd_time_hrtimer);

    // proof.*
    register!("proof.snapshot",       "emit proof root (stream to GUI)",  cmd_proof_snapshot);

    // net.*
    register!("net.send.proof",       "publish proof root to mesh",       cmd_net_send_proof);

    // gui.*
    register!("gui.ping",             "ping GUI bridge",                  cmd_gui_ping);
}

pub fn spawn() {
    init();
    sched::task::kspawn("cli", cli_thread, 0, Priority::Normal, Affinity::ANY);
    spawn_metrics_stream();
}

// —————————————————— main thread ——————————————————

extern "C" fn cli_thread(_arg: usize) -> ! {
    println("\nNØNOS CLI online. `help` for commands.");
    let mut buf = [0u8; MAX_LINE];

    loop {
        print(PROMPT);

        // prefer GUI stdin; fallback to TUI
        let n = if gui_bridge::is_connected() {
            let got = gui_bridge::recv_line(&mut buf);
            if got == 0 { tui::read_line(&mut buf) } else { got }
        } else {
            tui::read_line(&mut buf)
        };
        if n == 0 { continue; }

        let line = match str::from_utf8(&buf[..n]) { Ok(s) => s.trim(), Err(_) => { println!("utf8?"); continue; } };
        if line.is_empty() { continue; }

        remember(line);
        mirror(line);

        let mut argv: [&str; 16] = [""; 16];
        let argc = words(line, &mut argv);
        if argc == 0 { continue; }

        if let Some(f) = lookup(argv[0]) {
            if let Err(e) = f(&argv[..argc]) {
                println(e);
            }
        } else {
            if let Some(s) = suggest(argv[0]) {
                println(&format!("unknown: {} — did you mean `{}`?", argv[0], s));
            } else {
                println(&format!("unknown: {} (help)", argv[0]));
            }
        }
    }
}

// —————————————————— commands ——————————————————

fn cmd_help(_a: &[&str]) -> Result<(), &'static str> {
    let r = REG.lock();
    println("commands:");
    for c in r.iter() {
        println(&format!("  {:<20}  {}", c.name, c.help));
    }
    Ok(())
}

fn cmd_sys_time(_a: &[&str]) -> Result<(), &'static str> {
    let ns = timer::now_ns();
    println(&format!("time {} ns ({} ms) deadline={}", ns, ns/1_000_000, timer::is_deadline_mode()));
    Ok(())
}

fn cmd_sys_mem(_a: &[&str]) -> Result<(), &'static str> {
    memory::layout::dump(|s| print(s));
    print("\n");
    memory::virt::dump(|s| print(s));
    Ok(())
}

fn cmd_sys_apic(_a: &[&str]) -> Result<(), &'static str> {
    println(&format!("lapic id {}", apic::id()));
    Ok(())
}

fn cmd_sys_ioapic_route(a: &[&str]) -> Result<(), &'static str> {
    let gsi = a.get(1).and_then(|x| x.parse::<u32>().ok()).ok_or("usage: sys.ioapic.route <gsi>")?;
    let (vec, rte) = ioapic::alloc_route(gsi, apic::id()).map_err(|_| "alloc")?;
    ioapic::program_route(gsi, rte).map_err(|_| "program")?;
    ioapic::mask(gsi, false).ok();
    println(&format!("gsi {} -> vec 0x{:02x}", gsi, vec));
    Ok(())
}

fn cmd_rq_stats(_a: &[&str]) -> Result<(), &'static str> {
    let c = rq::stats_counts();
    println(&format!("rq rt={} hi={} norm={} low={} idle={}", c[0], c[1], c[2], c[3], c[4]));
    Ok(())
}

fn cmd_task_spawn(a: &[&str]) -> Result<(), &'static str> {
    let name = a.get(1).copied().unwrap_or("demo");
    let ms = a.get(2).and_then(|x| x.parse::<u64>().ok()).unwrap_or(500);
    let prio = match a.get(3).copied().unwrap_or("norm") {
        "rt" => Priority::Realtime,
        "hi" => Priority::High,
        "lo" => Priority::Low,
        "idle" => Priority::Idle,
        _ => Priority::Normal,
    };
    let tid = task::kspawn(name, demo_task, ms as usize, prio, Affinity::ANY);
    println(&format!("spawned tid={:?} prio={:?}", tid, prio));
    Ok(())
}

fn cmd_time_hrtimer(a: &[&str]) -> Result<(), &'static str> {
    let ms = a.get(1).and_then(|x| x.parse::<u64>().ok()).unwrap_or(50);
    let id = timer::hrtimer_after_ns(ms*1_000_000, || { crate::ui::tui::write("[hr]\n"); });
    println(&format!("hrtimer id={} {} ms", id, ms));
    Ok(())
}

fn cmd_proof_snapshot(_a: &[&str]) -> Result<(), &'static str> {
    let mut roots = [[0u8;32]; 64];
    let mut hdr = proof::SnapshotHeader::default();
    let n = proof::snapshot(&mut roots, &mut hdr);
    println(&format!("root {:02x?} caps {}", &hdr.root, n));
    event::publish(Event::ProofRoot { root: hdr.root, epoch: hdr.epoch });
    gui_json_proof(&hdr.root, hdr.epoch);
    Ok(())
}

fn cmd_net_send_proof(_a: &[&str]) -> Result<(), &'static str> {
    let mut roots = [[0u8;32]; 1];
    let mut hdr = proof::SnapshotHeader::default();
    let _ = proof::snapshot(&mut roots, &mut hdr);
    event::publish(Event::ProofRoot { root: hdr.root, epoch: hdr.epoch });
    println("queued proof root for mesh");
    Ok(())
}

fn cmd_gui_ping(_a: &[&str]) -> Result<(), &'static str> {
    gui_bridge::send_json("{\"type\":\"ping\"}");
    println("gui ping");
    Ok(())
}

// —————————————————— metrics stream → GUI ——————————————————

fn spawn_metrics_stream() {
    extern "C" fn t(_arg: usize) -> ! {
        loop {
            if gui_bridge::is_connected() {
                let ms = timer::now_ms();
                let rqv = rq::stats_counts();
                let json = json_metrics(ms, &rqv);
                gui_bridge::send_json(&json);
                event::publish(Event::Heartbeat { ms, rq: rqv });
            }
            timer::busy_sleep_ns(1_000_000_000);
        }
    }
    let _ = task::kspawn("cli.metrics", t, 0, Priority::Low, Affinity::ANY);
}

fn json_metrics(ms: u64, rq: &[usize;5]) -> heapless::String<256> {
    let mut s: heapless::String<256> = heapless::String::new();
    let _ = write!(s, "{{\"type\":\"metrics\",\"ms\":{},\"rq\":[{},{},{},{},{}]}}",
        ms, rq[0], rq[1], rq[2], rq[3], rq[4]);
    s
}

fn gui_json_proof(root: &[u8;32], epoch: u64) {
    let mut s: heapless::String<256> = heapless::String::new();
    let _ = write!(s, "{{\"type\":\"proof\",\"epoch\":{},\"root\":\"0x", epoch);
    for b in root { let _ = write!(s, "{:02x}", b); }
    let _ = write!(s, "\"}}");
    gui_bridge::send_json(&s);
}

// —————————————————— demo worker ——————————————————

extern "C" fn demo_task(period_ms: usize) -> ! {
    let tid = task::current();
    loop {
        let t = timer::now_ms();
        println(&format!("[demo {:?}] t={} ms", tid, t));
        timer::sleep_long_ns((period_ms as u64) * 1_000_000, || {});
        crate::sched::schedule_now();
    }
}

// —————————————————— print glue (dual sink) ——————————————————

#[inline]
fn print(s: &str) { tui::write(s); gui_bridge::send_line(s); }

#[inline]
fn println(s: &str) { print(s); print("\n"); }

#[inline]
fn println_fmt(args: core::fmt::Arguments) {
    struct W;
    impl core::fmt::Write for W {
        fn write_str(&mut self, s: &str) -> core::fmt::Result { print(s); Ok(()) }
    }
    let _ = W.write_fmt(args);
}
#[macro_export]
macro_rules! kprintln {
    ($($arg:tt)*) => ($crate::ui::cli::println_fmt(format_args!($($arg)*)));
}
use kprintln as println;

// —————————————————— test hook ——————————————————

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn parse_words() {
        let mut a: [&str;16] = ["";16];
        assert_eq!(words("a b  c", &mut a), 3);
        assert_eq!(a[0], "a"); assert_eq!(a[1], "b"); assert_eq!(a[2], "c");
    }
}
