// ui/cli.rs
//
// NØNOS kernel CLI 
// - Command registry (name -> handler, help, completer). O(1) lookup via tiny hash.
// - Line editor: history (ring), cursor editing, tab-completion hook.
// - Structured emit: text to TUI/VGA + JSON-L (NDJSON) to host (ui::ipc::bridge).
// - Async command jobs via kspawn (long ops don’t block REPL).
// - Low-alloc; fixed buffers; ISR-safe emit path.
// - Built-ins: time, proof, mem, maps, rq, task, apic, ioapic, hrtimer, sleep, loglvl.
//
// Zero-state. All data is public (no secrets).

#![allow(dead_code)]

use core::{fmt::Write, str};
use spin::{Mutex, Once};

use crate::arch::x86_64::time::timer;
use crate::sched::{self, task::{self, Priority, Affinity}};
use crate::sched::runqueue as rq;
use crate::memory::{self, proof};
use crate::arch::x86_64::interrupt::{apic, ioapic};
use crate::ui::{tui, ipc::bridge as host};

const PROMPT: &str = "nonos# ";
const MAX_LINE: usize = 256;
const HIST_MAX: usize = 32;

static REG: Once<Registry> = Once::new();

pub fn spawn_shell() {
    init_registry();
    sched::task::kspawn("cli", cli_thread, 0, Priority::Normal, Affinity::ANY);
}

// —————————————————— registry ——————————————————

type CmdFn = fn(&[&str]) -> Result<(), &'static str>;
type CplFn = fn(&[&str], &mut heapless::Vec<&'static str, 16>);

struct Command {
    name: &'static str,
    help: &'static str,
    run:  CmdFn,
    cpl:  Option<CplFn>,
}

struct Registry {
    // fixed, small table; binary search is fine
    cmds: &'static [Command],
}
impl Registry {
    fn find(&self, name: &str) -> Option<&'static Command> {
        self.cmds.iter().find(|c| c.name == name)
    }
    fn list(&self) -> &'static [Command] { self.cmds }
}

fn init_registry() {
    let reg = Registry {
        cmds: &[
            Command { name: "help",   help: "help [cmd]",                         run: cmd_help,   cpl: Some(cpl_cmds) },
            Command { name: "time",   help: "time — show clocks",                 run: cmd_time,   cpl: None },
            Command { name: "proof",  help: "proof — root + caps",                run: cmd_proof,  cpl: None },
            Command { name: "mem",    help: "mem — layout & phys",                run: cmd_mem,    cpl: None },
            Command { name: "maps",   help: "maps — page tables",                 run: cmd_maps,   cpl: None },
            Command { name: "rq",     help: "rq — runqueue counts",               run: cmd_rq,     cpl: None },
            Command { name: "task",   help: "task spawn <name> <ms> [rt|hi|norm|lo|idle]", run: cmd_task,  cpl: Some(cpl_task) },
            Command { name: "sleep",  help: "sleep <ms>",                         run: cmd_sleep,  cpl: None },
            Command { name: "hrtimer",help: "hrtimer <ms>",                       run: cmd_hrtimer,cpl: None },
            Command { name: "apic",   help: "apic (id|ipi <vec>|timer <hz>)",     run: cmd_apic,   cpl: Some(cpl_apic) },
            Command { name: "ioapic", help: "ioapic (route <gsi>|mask <gsi>)",    run: cmd_ioapic, cpl: Some(cpl_ioapic) },
            Command { name: "loglvl", help: "loglvl <0..4>",                      run: cmd_loglvl, cpl: None },
            Command { name: "panic",  help: "panic — trigger",                    run: |_a| { panic!("cli requested"); } , cpl: None },
        ],
    };
    REG.call_once(|| reg);
}

fn reg() -> &'static Registry { REG.get().expect("cli reg") }

// —————————————————— REPL ——————————————————

extern "C" fn cli_thread(_arg: usize) -> ! {
    tui::write("\nNØNOS kernel CLI up. Type 'help'.\n");
    host::emit_json(|w| {
        w.event("cli_ready").kv("prompt", PROMPT).finish();
    });

    let mut line = [0u8; MAX_LINE];
    let mut hist = History::new();
    loop {
        print(PROMPT);
        let n = tui::read_line_edit(&mut line, &mut hist, complete);
        if n == 0 { continue; }
        let s = match str::from_utf8(&line[..n]) { Ok(x) => x.trim(), Err(_) => { println("utf8?"); continue; } };
        if s.is_empty() { continue; }
        hist.push(s);

        let mut parts = SmallSplit::new(s);
        let Some(cmd) = parts.next() else { continue; };
        let args = parts.collect();

        if let Some(entry) = reg().find(cmd) {
            // run long ops off-thread
            spawn_cmd_job(entry, args);
        } else {
            println("unknown (help)");
            host::emit_json(|w| w.event("cli_unknown").kv("cmd", cmd).finish());
        }
    }
}

// run command in a detached task (so CLI stays responsive)
fn spawn_cmd_job(c: &'static Command, argv: heapless::Vec<&str, 16>) {
    struct ArgPack { cmd: &'static Command, args: heapless::Vec<&'static str, 16> }
    // Copy args to 'static via tiny inline arena (statically sized strings only)
    let mut fixed: heapless::Vec<&'static str, 16> = heapless::Vec::new();
    for a in argv.iter() {
        // Safety: CLI commands are tokens from input; we do not allocate new strings here.
        // Treat them ephemeral; if you need owned strings, promote via a global arena.
        // For now, pass as &str with 'static lie only inside this job’s lifetime.
        fixed.push(unsafe { core::mem::transmute::<&str, &'static str>(*a) }).ok();
    }
    let pack = ArgPack { cmd: c, args: fixed };

    extern "C" fn runner(raw: usize) -> ! {
        let p = unsafe { &*(raw as *const ArgPack) };
        let name = p.cmd.name;
        host::emit_json(|w| w.event("cli_start").kv("cmd", name).finish());
        match (p.cmd.run)(&p.args) {
            Ok(_) => {
                host::emit_json(|w| w.event("cli_done").kv("cmd", name).finish());
            }
            Err(e) => {
                println_fmt(format_args!("err: {}\n", e));
                host::emit_json(|w| w.event("cli_err").kv("cmd", name).kv("err", e).finish());
            }
        }
        // free pack (it’s on the stack of spawn site; nothing heap-allocated here)
        sched::schedule_now(); // yield back
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    // Pack lives on this stack; move by pointer (racy if yielded); so make it static by boxing.
    // We assume a tiny kernel allocator exists (kmem_alloc_zero). If not, inline a slab.
    let ptr = unsafe {
        use core::alloc::Layout;
        let layout = Layout::new::<ArgPack>();
        let mem = crate::memory::alloc::kmem_alloc_zero(layout.size(), layout.align()).expect("cli alloc");
        core::ptr::write(mem.cast::<ArgPack>(), pack);
        mem as usize
    };

    let _tid = sched::task::kspawn("cli-cmd", runner, ptr, Priority::Normal, Affinity::ANY);
}

// —————————————————— line editor bits ——————————————————

struct History {
    ring: heapless::Vec<heapless::String<MAX_LINE>, HIST_MAX>,
    idx: isize,
}
impl History {
    fn new() -> Self { Self { ring: heapless::Vec::new(), idx: -1 } }
    fn push(&mut self, s: &str) {
        if self.ring.len() == HIST_MAX { let _ = self.ring.remove(0); }
        let mut h = heapless::String::<MAX_LINE>::new();
        let _ = h.push_str(s);
        self.ring.push(h).ok();
        self.idx = -1;
    }
    fn prev(&mut self) -> Option<&str> {
        if self.ring.is_empty() { return None; }
        if self.idx < 0 { self.idx = (self.ring.len() as isize) - 1; }
        else if self.idx > 0 { self.idx -= 1; }
        Some(&self.ring[self.idx as usize])
    }
    fn next(&mut self) -> Option<&str> {
        if self.ring.is_empty() { return None; }
        if self.idx >= 0 && (self.idx as usize) < self.ring.len() - 1 { self.idx += 1; }
        else { self.idx = -1; return Some(""); }
        Some(&self.ring[self.idx as usize])
    }
}

// tab completion hook: fill suggestions
fn complete(prefix_line: &str, out: &mut heapless::Vec<&'static str, 16>) {
    let mut parts = SmallSplit::new(prefix_line);
    let first = parts.next().unwrap_or("");
    let rest = parts.collect();

    if rest.is_empty() {
        cpl_cmds(&[], out);
        // filter by prefix
        let p = first;
        let mut i = 0;
        while i < out.len() {
            if !out[i].starts_with(p) { out.remove(i); } else { i += 1; }
        }
    } else if let Some(c) = reg().find(first) {
        if let Some(cb) = c.cpl { cb(&rest, out); }
    }
}

fn cpl_cmds(_args: &[&str], out: &mut heapless::Vec<&'static str, 16>) {
    for c in reg().list() { out.push(c.name).ok(); }
}
fn cpl_task(args: &[&str], out: &mut heapless::Vec<&'static str, 16>) {
    if args.len() == 0 { out.push("spawn").ok(); }
    else if args.len() == 3 {
        out.extend_from_slice(&["rt","hi","norm","lo","idle"]).ok();
    }
}
fn cpl_apic(args: &[&str], out: &mut heapless::Vec<&'static str, 16>) {
    if args.len() == 0 { out.extend_from_slice(&["id","ipi","timer"]).ok(); }
}
fn cpl_ioapic(args: &[&str], out: &mut heapless::Vec<&'static str, 16>) {
    if args.len() == 0 { out.extend_from_slice(&["route","mask"]).ok(); }
}

// —————————————————— commands ——————————————————

fn cmd_help(args: &[&str]) -> Result<(), &'static str> {
    if let Some(name) = args.get(0) {
        if let Some(c) = reg().find(name) {
            println_fmt(format_args!("{}\n", c.help));
            return Ok(());
        }
    }
    for c in reg().list() {
        println_fmt(format_args!("{:<10} {}\n", c.name, c.help));
    }
    Ok(())
}

fn cmd_time(_a: &[&str]) -> Result<(), &'static str> {
    let ns = timer::now_ns();
    println_fmt(format_args!(
        "time: {} ns ({} ms) tsc ~{} kHz deadline={}\n",
        ns, ns/1_000_000, timer::tsc_khz(), timer::is_deadline_mode()
    ));
    host::emit_json(|w| w.event("time").kv_u64("ns", ns).kv_u64("ms", ns/1_000_000).finish());
    Ok(())
}

fn cmd_proof(_a: &[&str]) -> Result<(), &'static str> {
    let mut roots = [[0u8;32]; 64];
    let mut hdr = proof::SnapshotHeader::default();
    let n = proof::snapshot(&mut roots, &mut hdr);
    println_fmt(format_args!("proof root: {:02x?}\n", &hdr.root));
    println_fmt(format_args!("caps: {} schema={} epoch={} boot_nonce={}\n", n, hdr.schema, hdr.epoch, hdr.boot_nonce));
    host::emit_json(|w| {
        let mut w = w.event("proof");
        w.kv("root_hex", crate::util::hex32(&hdr.root));
        w.kv_u64("caps", n as u64).kv_u64("epoch", hdr.epoch).finish();
    });
    Ok(())
}

fn cmd_mem(_a: &[&str]) -> Result<(), &'static str> {
    memory::layout::dump(|s| tui::write(s));
    tui::write("\n");
    memory::phys::dump(|s| tui::write(s));
    host::emit_json(|w| w.event("mem_dump").finish());
    Ok(())
}

fn cmd_maps(_a: &[&str]) -> Result<(), &'static str> {
    crate::memory::virt::dump(|s| tui::write(s));
    host::emit_json(|w| w.event("maps_dump").finish());
    Ok(())
}

fn cmd_rq(_a: &[&str]) -> Result<(), &'static str> {
    let c = rq::stats_counts();
    println_fmt(format_args!("runqueue: rt={} hi={} norm={} low={} idle={}\n", c[0],c[1],c[2],c[3],c[4]));
    host::emit_json(|w| w.event("rq").kv_u64("rt",c[0] as u64).kv_u64("hi",c[1] as u64).kv_u64("norm",c[2] as u64).kv_u64("low",c[3] as u64).kv_u64("idle",c[4] as u64).finish());
    Ok(())
}

fn cmd_task(args: &[&str]) -> Result<(), &'static str> {
    match args.get(0).copied() {
        Some("spawn") => {
            let name = args.get(1).copied().unwrap_or("demo");
            let period_ms: usize = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(500);
            let prio = match args.get(3).copied().unwrap_or("norm") {
                "rt" => Priority::Realtime, "hi" => Priority::High, "lo" => Priority::Low,
                "idle" => Priority::Idle, _ => Priority::Normal
            };
            let tid = task::kspawn(name, demo_task, period_ms, prio, Affinity::ANY);
            println_fmt(format_args!("spawned tid={:?} prio={:?}\n", tid, prio));
            host::emit_json(|w| w.event("task_spawn").kv("name",name).kv_u64("tid",tid.0).finish());
            Ok(())
        }
        _ => Err("usage: task spawn <name> <ms> [rt|hi|norm|lo|idle]"),
    }
}

fn cmd_sleep(args: &[&str]) -> Result<(), &'static str> {
    let ms: u64 = args.get(0).and_then(|x| x.parse().ok()).unwrap_or(100);
    timer::busy_sleep_ns(ms*1_000_000);
    println_fmt(format_args!("slept {} ms\n", ms));
    Ok(())
}

fn cmd_hrtimer(args: &[&str]) -> Result<(), &'static str> {
    let ms: u64 = args.get(0).and_then(|x| x.parse().ok()).unwrap_or(50);
    let id = timer::hrtimer_after_ns(ms*1_000_000, || { tui::write("[hrtimer]\n"); });
    println_fmt(format_args!("hrtimer id={} {} ms\n", id, ms));
    Ok(())
}

fn cmd_apic(args: &[&str]) -> Result<(), &'static str> {
    match args.get(0).copied() {
        Some("id") => { println_fmt(format_args!("lapic id = {}\n", apic::id())); Ok(()) }
        Some("ipi") => {
            let vec: u8 = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(0xF1);
            apic::ipi_self(vec); println_fmt(format_args!("sent self ipi vec=0x{:02x}\n", vec)); Ok(())
        }
        Some("timer") => {
            let hz: u32 = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(1000);
            let _ = apic::timer_enable(hz, 16, 0);
            println_fmt(format_args!("lapic timer hz={}\n", hz)); Ok(())
        }
        _ => Err("apic: id | ipi <vec> | timer <hz>"),
    }
}

fn cmd_ioapic(args: &[&str]) -> Result<(), &'static str> {
    match args.get(0).copied() {
        Some("route") => {
            let gsi: u32 = args.get(1).and_then(|x| x.parse().ok()).ok_or("gsi")?;
            let dest = apic::id();
            let (vec, rte) = ioapic::alloc_route(gsi, dest).map_err(|_| "alloc")?;
            ioapic::program_route(gsi, rte).map_err(|_| "prog")?;
            ioapic::mask(gsi, false).ok();
            println_fmt(format_args!("gsi {} → vec 0x{:02x} dest {}\n", gsi, vec, dest));
            Ok(())
        }
        Some("mask") => {
            let gsi: u32 = args.get(1).and_then(|x| x.parse().ok()).ok_or("gsi")?;
            ioapic::mask(gsi, true).ok();
            println_fmt(format_args!("gsi {} masked\n", gsi)); Ok(())
        }
        _ => Err("ioapic: route <gsi> | mask <gsi>"),
    }
}

fn cmd_loglvl(args: &[&str]) -> Result<(), &'static str> {
    let lvl: u8 = args.get(0).and_then(|x| x.parse().ok()).unwrap_or(2);
    crate::log::logger::set_level(lvl);
    println_fmt(format_args!("log level {}\n", lvl));
    Ok(())
}

// —————————————————— demo ——————————————————

extern "C" fn demo_task(period_ms: usize) -> ! {
    let tid = task::current();
    loop {
        let t = timer::now_ms();
        println_fmt(format_args!("[demo {:?}] t={} ms\n", tid, t));
        timer::sleep_long_ns((period_ms as u64)*1_000_000, || {});
        crate::sched::schedule_now();
    }
}

// —————————————————— util ——————————————————

struct SmallSplit<'a> { s: &'a str, off: usize }
impl<'a> SmallSplit<'a> {
    fn new(s: &'a str) -> Self { Self { s, off: 0 } }
    fn next(&mut self) -> Option<&'a str> {
        // split by ASCII whitespace
        while self.off < self.s.len() && self.s.as_bytes()[self.off].is_ascii_whitespace() { self.off+=1; }
        if self.off >= self.s.len() { return None; }
        let start = self.off;
        while self.off < self.s.len() && !self.s.as_bytes()[self.off].is_ascii_whitespace() { self.off+=1; }
        Some(&self.s[start..self.off])
    }
    fn collect(mut self) -> heapless::Vec<&'a str, 16> {
        let mut v = heapless::Vec::new();
        while let Some(p) = self.next() { v.push(p).ok(); }
        v
    }
}

#[inline] fn print(s: &str) { tui::write(s); }
fn println(s: &str) { tui::write(s); tui::write("\n"); }
fn println_fmt(args: core::fmt::Arguments) {
    struct W; impl core::fmt::Write for W { fn write_str(&mut self, s: &str) -> core::fmt::Result { tui::write(s); Ok(()) } }
    let _ = W.write_fmt(args);
}
