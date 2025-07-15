pub mod measurement { #[repr(C)] pub struct BootInfo<'a>{ pub mem_map: &'a [usize], pub kernel_sz: usize, pub kernel_sha: [u8;32] } }
