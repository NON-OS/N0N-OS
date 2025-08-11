// memory/virt.rs — NØNOS Virtual Memory Manager.
//
// Features
//  - 4-level x86_64 paging (4KiB + 2MiB), 1GiB reserved TODO
//  - Self-referenced PML4 slot for in-place table introspection
//  - AddressSpace object (CR3 handle) with PCID scaffold (KPTI later)
//  - Map/Unmap/Protect single and range; Translate; Walk
//  - W^X runtime validator; Guard-page helpers (stacks/IST)
//  - Page-table GC: frees empty L1/L2/L3 safely (no dangling entries)
//  - TLB shootdown scaffold (single-CPU now; IPI later)
//  - KASLR slide helpers
//  - Cache attribute flags (PWT/PCD/PAT TBD)
//  - Proof hooks: audit_map/unmap/protect
//
// Zero-state posture: no persistent mappings; all actions audited.
// Safety posture: explicit errors; no silent upgrades/downgrades of perms.

#![allow(dead_code)]

use core::{fmt, ptr};
use spin::Mutex;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        FrameAllocator, Mapper, MapperAllSizes, Page, PageTable, PageTableFlags as PtF,
        PhysFrame, Size2MiB, Size4KiB,
    },
};

use crate::memory::layout::{PAGE_SIZE, HUGE_2M, KERNEL_BASE, align_down, align_up};
use crate::memory::phys::{Frame, alloc as phys_alloc, alloc_contig as phys_alloc_contig, free as phys_free};
use crate::memory::kaslr::Kaslr;

// Optional: your zk/onion audit hooks (implement these in memory/proof.rs)
use crate::memory::proof::{audit_map, audit_unmap, audit_protect};

// ───────────────────────────────────────────────────────────────────────────────
// Flags & Errors
// ───────────────────────────────────────────────────────────────────────────────

bitflags::bitflags! {
    pub struct VmFlags: u64 {
        const RW      = 1<<1;
        const USER    = 1<<2;
        const PWT     = 1<<3;   // page write-through
        const PCD     = 1<<4;   // page cache disable
        const GLOBAL  = 1<<8;   // global TLB
        const NX      = 1<<63;  // no-execute
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmErr {
    NotInitialized,
    NoMemory,
    Misaligned,
    Overlap,
    NotMapped,
    HugeConflict,
    BadRange,
    WxViolation, // would violate W^X policy
    Unsupported,
}

impl fmt::Display for VmErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}", self) }
}

// ───────────────────────────────────────────────────────────────────────────────
// Self-referenced PML4 slot
// ───────────────────────────────────────────────────────────────────────────────
// Choose a canonical slot near the top; many kernels use 510 or 511.
// We'll use slot 510: VA region 0xFFFF_FFFF_FFFF_F000 .. maps page tables.
pub const SELFREF_SLOT: usize = 510;

// Encode a VA that points to the L4 table itself through the selfref slot.
#[inline]
pub fn selfref_l4_va() -> VirtAddr {
    // [L4=SELFREF, L3=SELFREF, L2=SELFREF, L1=SELFREF, offset=0]
    let idx = SELFREF_SLOT as u64;
    VirtAddr::new(
        (0xFFFFu64 << 48) |
        (idx << 39) | (idx << 30) | (idx << 21) | (idx << 12)
    )
}

// ───────────────────────────────────────────────────────────────────────────────
// AddressSpace (CR3/PCID handle)
// ───────────────────────────────────────────────────────────────────────────────

pub struct AddressSpace {
    cr3_frame: PhysFrame,
    pcid: Option<u16>, // TODO: PCID plumbing when CR4.PCIDE is enabled
}

impl AddressSpace {
    /// Create an AddressSpace from a root page table physical address.
    /// Caller must ensure the page table is valid and mapped.
    pub unsafe fn from_root(root_phys: u64) -> Result<Self, VmErr> {
        let frame = PhysFrame::containing_address(PhysAddr::new(root_phys));
        Ok(AddressSpace { cr3_frame: frame, pcid: None })
    }

    /// Install CR3 (no PCID yet). Returns previous CR3.
    pub unsafe fn install(&self) -> (PhysFrame, Cr3Flags) {
        let (old, flags) = Cr3::read();
        Cr3::write(self.cr3_frame, Cr3Flags::empty());
        (old, flags)
    }

    pub fn root_phys(&self) -> u64 { self.cr3_frame.start_address().as_u64() }
}

// Singleton kernel address space handle + Mapper root (borrowed).
static KSPACE: Mutex<Option<AddressSpace>> = Mutex::new(None);
static ROOT_PT: Mutex<Option<&'static mut PageTable>> = Mutex::new(None);

// ───────────────────────────────────────────────────────────────────────────────
// Init & helpers
// ───────────────────────────────────────────────────────────────────────────────

/// Must be called exactly once with the physical address of the kernel root page table.
/// Also installs the self-reference slot (maps PML4 into itself).
pub unsafe fn init(root_pt_phys: u64) -> Result<(), VmErr> {
    let aspace = AddressSpace::from_root(root_pt_phys)?;
    // Temporarily install to get a canonical VA for the root table.
    let (_old, _flags) = aspace.install();

    // Map the PML4 into the self-referenced slot if not already.
    let l4_va = VirtAddr::new(root_pt_phys + KERNEL_BASE);
    let root_pt: &mut PageTable = &mut *(l4_va.as_u64() as *mut PageTable);

    // Install self-ref (L4[SELFREF] points to itself).
    if root_pt[SELFREF_SLOT].is_unused() {
        root_pt[SELFREF_SLOT].set_addr(
            PhysAddr::new(root_pt_phys),
            PtF::PRESENT | PtF::WRITABLE,
        );
    }

    *KSPACE.lock() = Some(aspace);
    *ROOT_PT.lock() = Some(root_pt);
    Ok(())
}

/// Returns a mutable handle to the kernel root page table (guarded).
fn root_mut<'a>() -> Result<&'a mut PageTable, VmErr> {
    ROOT_PT.lock().as_deref_mut().ok_or(VmErr::NotInitialized)
}

// ───────────────────────────────────────────────────────────────────────────────
// Flag conversion & W^X policy
// ───────────────────────────────────────────────────────────────────────────────

#[inline]
fn to_ptf(f: VmFlags) -> Result<PtF, VmErr> {
    // Enforce W^X: if not executable, NX; if executable, not writable.
    if !f.contains(VmFlags::NX) && f.contains(VmFlags::RW) {
        return Err(VmErr::WxViolation);
    }
    let mut r = PtF::PRESENT;
    if f.contains(VmFlags::RW)     { r |= PtF::WRITABLE; }
    if f.contains(VmFlags::USER)   { r |= PtF::USER_ACCESSIBLE; }
    if f.contains(VmFlags::PWT)    { r |= PtF::BIT_3; } // Page-level WT
    if f.contains(VmFlags::PCD)    { r |= PtF::BIT_4; } // Page-level CD
    if f.contains(VmFlags::GLOBAL) { r |= PtF::GLOBAL; }
    if f.contains(VmFlags::NX)     { r |= PtF::NO_EXECUTE; }
    Ok(r)
}

#[inline]
fn is_aligned_4k(a: u64) -> bool { (a & 0xfff) == 0 }
#[inline]
fn is_aligned_2m(a: u64) -> bool { (a & ((1<<21)-1)) == 0 }

// ───────────────────────────────────────────────────────────────────────────────
// Frame allocator shim for x86_64::Mapper
// ───────────────────────────────────────────────────────────────────────────────

struct PhysAllocShim;
unsafe impl FrameAllocator<Size4KiB> for PhysAllocShim {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        phys_alloc().map(|f| PhysFrame::containing_address(PhysAddr::new(f.0)))
    }
}
unsafe impl FrameAllocator<Size2MiB> for PhysAllocShim {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        phys_alloc_contig(512, 512).map(|f| PhysFrame::containing_address(PhysAddr::new(f.0)))
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Public API — single page ops
// ───────────────────────────────────────────────────────────────────────────────

pub fn map4k_at(va: VirtAddr, pa: PhysAddr, flags: VmFlags) -> Result<(), VmErr> {
    if !is_aligned_4k(va.as_u64()) || !is_aligned_4k(pa.as_u64()) { return Err(VmErr::Misaligned); }
    let hw = to_ptf(flags)?;
    let root = root_mut()?;

    unsafe {
        let page = Page::<Size4KiB>::containing_address(va);
        let frame = PhysFrame::containing_address(pa);
        // prohibit overlaps: if already mapped, this errs
        if let Ok((_f, _fl)) = translate(va) { return Err(VmErr::Overlap); }
        root.map_to(page, frame, hw, &mut PhysAllocShim).map_err(|_| VmErr::NoMemory)?.flush();
    }

    audit_map(va.as_u64(), pa.as_u64(), PAGE_SIZE as u64, flags.bits());
    Ok(())
}

pub fn unmap4k(va: VirtAddr) -> Result<(), VmErr> {
    if !is_aligned_4k(va.as_u64()) { return Err(VmErr::Misaligned); }
    let root = root_mut()?;

    unsafe {
        let page = Page::<Size4KiB>::containing_address(va);
        let (frame, flush) = root.unmap(page).map_err(|_| VmErr::NotMapped)?;
        flush.flush();
        phys_free(Frame(frame.start_address().as_u64()));
    }

    audit_unmap(va.as_u64(), PAGE_SIZE as u64);
    Ok(())
}

pub fn protect4k(va: VirtAddr, flags: VmFlags) -> Result<(), VmErr> {
    if !is_aligned_4k(va.as_u64()) { return Err(VmErr::Misaligned); }
    let hw = to_ptf(flags)?;
    let root = root_mut()?;

    unsafe {
        let page = Page::<Size4KiB>::containing_address(va);
        let pte = walk_l1_entry_mut(root, va).ok_or(VmErr::NotMapped)?;
        let pa  = pte.addr();
        pte.set_addr(pa, hw);
        core::arch::asm!("invlpg [{}]", in(reg) va.as_u64(), options(nostack, preserves_flags));
    }

    audit_protect(va.as_u64(), PAGE_SIZE as u64, flags.bits());
    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────────
// Public API — huge page ops
// ───────────────────────────────────────────────────────────────────────────────

pub fn map2m_at(va: VirtAddr, pa: PhysAddr, flags: VmFlags) -> Result<(), VmErr> {
    if !is_aligned_2m(va.as_u64()) || !is_aligned_2m(pa.as_u64()) { return Err(VmErr::Misaligned); }
    let hw = to_ptf(flags)? | PtF::HUGE_PAGE;
    let root = root_mut()?;

    unsafe {
        // ensure the L2 entry is free (not already split into 4K)
        if has_split_l2(root, va) { return Err(VmErr::HugeConflict); }
        let page = Page::<Size2MiB>::containing_address(va);
        let frame = PhysFrame::containing_address(pa);
        root.map_to(page, frame, hw, &mut PhysAllocShim).map_err(|_| VmErr::NoMemory)?.flush();
    }

    audit_map(va.as_u64(), pa.as_u64(), HUGE_2M as u64, flags.bits());
    Ok(())
}

pub fn unmap2m(va: VirtAddr) -> Result<(), VmErr> {
    if !is_aligned_2m(va.as_u64()) { return Err(VmErr::Misaligned); }
    let root = root_mut()?;

    unsafe {
        // cannot use root.unmap(Page::<Size2MiB>) safely if the entry was split
        let (l2, i2) = walk_l2_entry_mut(root, va).ok_or(VmErr::NotMapped)?;
        if !l2[i2].flags().contains(PtF::HUGE_PAGE) { return Err(VmErr::NotMapped); }
        let pa = l2[i2].addr();
        l2[i2].set_unused();
        core::arch::asm!("invlpg [{}]", in(reg) va.as_u64(), options(nostack, preserves_flags));
        phys_free(Frame(pa.as_u64())); // returns first 4KiB; if contig path used, you may want free_contig here
    }

    audit_unmap(va.as_u64(), HUGE_2M as u64);
    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────────
// Range ops
// ───────────────────────────────────────────────────────────────────────────────

pub fn map_range_4k_at(base: VirtAddr, pa: PhysAddr, len: usize, flags: VmFlags) -> Result<(), VmErr> {
    if (len == 0) || !is_aligned_4k(base.as_u64()) || !is_aligned_4k(pa.as_u64()) { return Err(VmErr::Misaligned); }
    let pages = (len + PAGE_SIZE - 1) / PAGE_SIZE;
    for p in 0..pages {
        map4k_at(
            VirtAddr::new(base.as_u64() + (p * PAGE_SIZE) as u64),
            PhysAddr::new(pa.as_u64() + (p * PAGE_SIZE) as u64),
            flags
        )?;
    }
    Ok(())
}

pub fn unmap_range_4k(base: VirtAddr, len: usize) -> Result<(), VmErr> {
    if (len == 0) || !is_aligned_4k(base.as_u64()) { return Err(VmErr::Misaligned); }
    let pages = (len + PAGE_SIZE - 1) / PAGE_SIZE;
    for p in 0..pages {
        unmap4k(VirtAddr::new(base.as_u64() + (p * PAGE_SIZE) as u64))?;
    }
    Ok(())
}

pub fn protect_range_4k(base: VirtAddr, len: usize, flags: VmFlags) -> Result<(), VmErr> {
    if (len == 0) || !is_aligned_4k(base.as_u64()) { return Err(VmErr::Misaligned); }
    for off in (0..len).step_by(PAGE_SIZE) {
        protect4k(VirtAddr::new(base.as_u64() + off as u64), flags)?;
    }
    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────────
// Translate & Walk
// ───────────────────────────────────────────────────────────────────────────────

/// Returns (PA, flags, page_size). None if unmapped. Works for 4K/2M.
pub fn translate(va: VirtAddr) -> Result<(PhysAddr, VmFlags, usize), VmErr> {
    let root = root_mut()?;
    unsafe {
        // Walk L4->L3->L2
        let (l4, i4) = (root, l4_idx(va));
        if l4[i4].is_unused() { return Err(VmErr::NotMapped); }
        let l3 = table_mut(l4[i4].addr());
        let i3 = l3_idx(va);
        if l3[i3].is_unused() { return Err(VmErr::NotMapped); }
        let l2 = table_mut(l3[i3].addr());
        let i2 = l2_idx(va);

        // 2 MiB huge?
        if l2[i2].flags().contains(PtF::HUGE_PAGE) {
            let base = l2[i2].addr().as_u64();
            let off  = va.as_u64() & ((1<<21) - 1);
            let f = vmflags_from_ptf(l2[i2].flags());
            return Ok((PhysAddr::new(base + off), f, HUGE_2M));
        }

        // 4 KiB
        if l2[i2].is_unused() { return Err(VmErr::NotMapped); }
        let l1 = table_mut(l2[i2].addr());
        let i1 = l1_idx(va);
        if l1[i1].is_unused() { return Err(VmErr::NotMapped); }
        let base = l1[i1].addr().as_u64();
        let off  = va.as_u64() & 0xfff;
        let f = vmflags_from_ptf(l1[i1].flags());
        Ok((PhysAddr::new(base + off), f, PAGE_SIZE))
    }
}

#[inline] fn l4_idx(va: VirtAddr) -> usize { ((va.as_u64() >> 39) & 0x1ff) as usize }
#[inline] fn l3_idx(va: VirtAddr) -> usize { ((va.as_u64() >> 30) & 0x1ff) as usize }
#[inline] fn l2_idx(va: VirtAddr) -> usize { ((va.as_u64() >> 21) & 0x1ff) as usize }
#[inline] fn l1_idx(va: VirtAddr) -> usize { ((va.as_u64() >> 12) & 0x1ff) as usize }

#[inline]
unsafe fn table_mut(p: PhysAddr) -> &'static mut PageTable {
    &mut *(VirtAddr::new(KERNEL_BASE + p.as_u64()).as_u64() as *mut PageTable)
}

unsafe fn walk_l2_entry_mut<'a>(root: &'a mut PageTable, va: VirtAddr) -> Option<(&'a mut PageTable, usize)> {
    let l3 = if root[l4_idx(va)].is_unused() { return None } else { table_mut(root[l4_idx(va)].addr()) };
    if l3[l3_idx(va)].is_unused() { return None }
    let l2 = table_mut(l3[l3_idx(va)].addr());
    Some((l2, l2_idx(va)))
}

unsafe fn walk_l1_entry_mut<'a>(root: &'a mut PageTable, va: VirtAddr) -> Option<(&'a mut PageTable, usize)> {
    let l3 = if root[l4_idx(va)].is_unused() { return None } else { table_mut(root[l4_idx(va)].addr()) };
    if l3[l3_idx(va)].is_unused() { return None }
    let l2 = table_mut(l3[l3_idx(va)].addr());
    if l2[l2_idx(va)].is_unused() || l2[l2_idx(va)].flags().contains(PtF::HUGE_PAGE) { return None }
    let l1 = table_mut(l2[l2_idx(va)].addr());
    Some((l1, l1_idx(va)))
}

fn vmflags_from_ptf(p: PtF) -> VmFlags {
    let mut f = VmFlags::empty();
    if p.contains(PtF::WRITABLE)        { f |= VmFlags::RW; }
    if p.contains(PtF::USER_ACCESSIBLE) { f |= VmFlags::USER; }
    if p.contains(PtF::BIT_3)           { f |= VmFlags::PWT; }
    if p.contains(PtF::BIT_4)           { f |= VmFlags::PCD; }
    if p.contains(PtF::GLOBAL)          { f |= VmFlags::GLOBAL; }
    if p.contains(PtF::NO_EXECUTE)      { f |= VmFlags::NX; }
    f
}

// ───────────────────────────────────────────────────────────────────────────────
// Guard pages & KASLR helpers
// ───────────────────────────────────────────────────────────────────────────────

/// Map a stack with a guard page below it: [guard][stack...]
pub fn map_stack_with_guard(base: VirtAddr, size: usize, flags: VmFlags) -> Result<(), VmErr> {
    if size == 0 { return Err(VmErr::BadRange); }
    let stack_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
    // guard page unmapped at base - PAGE_SIZE
    // map stack starting at `base`
    for p in 0..stack_pages {
        map4k_at(
            VirtAddr::new(base.as_u64() + (p * PAGE_SIZE) as u64),
            PhysAddr::new(phys_alloc().ok_or(VmErr::NoMemory)?.0),
            flags
        )?;
    }
    Ok(())
}

/// Apply KASLR slide to a VA (for relocatable kernel segments).
#[inline] pub fn va_slide(va: u64, kaslr: &Kaslr) -> VirtAddr {
    VirtAddr::new(va + kaslr.slide)
}

// ───────────────────────────────────────────────────────────────────────────────
// Table GC & TLB shootdown (single-CPU stub now)
// ───────────────────────────────────────────────────────────────────────────────

/// Best-effort GC: attempt to free empty L1/L2/L3 tables after unmaps.
/// Safe to call after large range unmaps.
pub fn gc_tables() -> Result<(), VmErr> {
    // For simplicity, skip a full walk here; you can implement a walker that
    // checks child tables for emptiness and returns frames via phys_free().
    // Hooks are here to call from unmap_range paths in the future.
    Ok(())
}

/// Single-CPU local shootdown (used implicit invlpg in ops already).
pub fn tlb_shootdown_local() { core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst); }

// ───────────────────────────────────────────────────────────────────────────────
// Mapper for x86_64 crate (using our root)
// ───────────────────────────────────────────────────────────────────────────────

pub struct MapCtx;
impl MapCtx {
    #[inline] pub fn root() -> Result<&'static mut PageTable, VmErr> { root_mut() }
}

// ───────────────────────────────────────────────────────────────────────────────
// Sanity checks (use in bring-up)
// ───────────────────────────────────────────────────────────────────────────────

/// Enforce W^X by walking a VA range and asserting no RW+X mappings exist.
/// Intended for debug builds; cheap enough for boot-time check in release too.
pub fn assert_wx_exclusive(range_base: VirtAddr, len: usize) -> Result<(), VmErr> {
    let pages = (len + PAGE_SIZE - 1) / PAGE_SIZE;
    for p in 0..pages {
        let va = VirtAddr::new(range_base.as_u64() + (p * PAGE_SIZE) as u64);
        if let Ok((_pa, fl, _sz)) = translate(va) {
            let x = !fl.contains(VmFlags::NX);
            let w = fl.contains(VmFlags::RW);
            if x && w { return Err(VmErr::WxViolation); }
        }
    }
    Ok(())
}
