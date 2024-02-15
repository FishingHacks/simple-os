use bootloader::BootInfo;
use x86_64::VirtAddr;

use crate::{
    allocator, gdt, interrupts,
    mem::{self, BootInfoFrameAllocator},
    print, println, vga_buffer, VERSION,
};

pub fn init_memory(boot_info: &'static BootInfo) {
    print_init_start("Memory");
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { mem::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    print_init_end("Memory");
    print_init_start("Heap");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");
    print_init_end("Heap");
}

pub fn print_init_start(name: &str) {
    print!("Initializing {name}...");
}

pub fn print_init_end(name: &str) {
    for _ in (0..(vga_buffer::BUFFER_WIDTH - 20).saturating_sub(name.len())).map(|_| ' ') {
        print!(" ");
    }
    println!("[ok]");
}

pub fn init_<F>(f: F, name: &str)
where
    F: Fn() -> (),
{
    print_init_start(name);
    f();
    print_init_end(name);
}

pub fn shared_init() {
    println!("SkyOS v{}", VERSION);

    // interrupts
    init_(interrupts::init_idt, "interrupts");
    init_(gdt::init, "gdt");
    init_(
        || unsafe { interrupts::PICS.lock().initialize() },
        "Hardware interrupts",
    );
    x86_64::instructions::interrupts::enable();
}
