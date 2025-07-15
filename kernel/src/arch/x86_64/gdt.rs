use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
static mut GDT: Option<GlobalDescriptorTable> = None;

pub fn init() {
    unsafe {
        GDT = Some(GlobalDescriptorTable::new());
        GDT.as_mut().unwrap().add_entry(Descriptor::kernel_code_segment());
        GDT.as_ref().unwrap().load();
    }
}
