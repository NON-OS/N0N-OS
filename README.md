# NØNOS OS

> The Trustless Operating System  
> Zero-state. Cryptographic. Terminal-native. Built from scratch.

 ███╗   ██╗ ██████╗  ███╗   ██╗ ██████╗ ███████╗
 ████╗  ██║██╔═══██╗████╗  ██║██╔═══██╗██╔════╝
 ██╔██╗ ██║██║   ██║██╔██╗ ██║██║   ██║███████╗
 ██║╚██╗██║██║   ██║██║╚██╗██║██║   ██║╚════██║
 ██║ ╚████║╚██████╔╝██║ ╚████║╚██████╔╝███████║
 ╚═╝  ╚═══╝ ╚═════╝ ╚═╝  ╚═══╝ ╚═════╝ ╚══════╝
 
---

### 🧠 What is NØNOS?

NØNOS is a cryptographically verifiable, sovereign operating system designed for zero-trust computing.  
It is engineered from the ground up — without Linux, without persistent identity, and with no reliance on centralized services or clouds.

---

### 🚀 Architecture Highlights

| Layer         | Stack                                   | Status        |
|---------------|------------------------------------------|----------------|
| **Bootloader**| UEFI binary (Rust, no GRUB)              | ✅ Working      |
| Kernel    | Pure RAM-only VGA stub                   | ✅ Working      |
| CLI       | nonosctl command line interface        | ✅ Stub ready   |
| Crypto    | ed25519 + blake3                         | 🔜 Next         |
| Modules   | Signed, sandboxed WASM-style payloads    | 🔜 Planned      |
| Network   | Encrypted relay via Anyone SDK           | 🔜 Planned      |

---

### 📦 Folder Structure

nonos-dev/
├── boot/       # UEFI bootloader
├── kernel/     # RAM-only secure kernel
├── cli/        # nonosctl CLI interface
├── crypto/     # ed25519 signature + hashing
├── net/        # Anyone SDK relay integration
├── modules/    # Signed ephemeral apps
├── docs/       # Architecture, specs, whitepaper


### 🔐 Core Principles

- Zero-State Mode: No disk writes, runs fully in RAM
- Terminal-First: No GUI, no telemetry, no daemons
- Cryptographically Verified: Every module signed
- Decentralized Networking: Anyone relay integration
- User ≠ Identity: No accounts. No cookies. No trackers.

---

### 🔧 Built With

- 💻 Rust (safe low-level kernel & UEFI)
- 🧬 UEFI-native bootloader (no GRUB)
- 🔐 ed25519 + blake3 crypto primitives
- 🛰️ Anyone SDK for encrypted relay routing
- 🧠 QEMU + USB test builds

---

### 🚧 Status: Alpha – Day 1 Complete ✅

We are currently in Alpha Phase, with the following completed:

- [x] Custom UEFI bootloader built & tested
- [x] Memory-only kernel stub runs in VGA
- [x] nonosctl CLI scaffolded
- [ ] Bootloader-to-kernel jump
- [ ] Module loader + crypto verification
- [ ] Relay communication over anonymous circuit

---

### 📖 Documentation

See full whitepaper + design logs in docs/ (SOON)
---

### 🧠 Philosophy

> "Infrastructure should be owned, not rented.  
> Computation should be verifiable, not trusted.  
> Identity should be optional, not assumed."  

NØNOS is not a remix.  
It’s a clean-room operating environment designed for sovereign terminals, anonymous coordination, and post-cloud computing.

---

### 💬 Credits

Built by eK, the creator of NONOS. 
  
Powered by caffeine, conviction, and cryptography ☕🧠🔐

---

### 🌐 License

You own what you compute. License: TBD.
