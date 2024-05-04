#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(skyos::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use skyos::pci::PCIManager;
use core::panic::PanicInfo;
use skyos::cmdline::CMD_LINE;
use skyos::vga_buffer::enable_cursor;
use skyos::{hlt_loop, init_memory, println, shared_init};
use x86_64::instructions::interrupts::without_interrupts;

fn run(boot_info: &'static BootInfo) {
    enable_cursor();
    shared_init();
    init_memory(boot_info);
    
    PCIManager::new().scan();

    without_interrupts(|| CMD_LINE.lock().init());

    hlt_loop();
}

fn panic_handler(info: &PanicInfo) -> ! {
    println!("{info}");
    skyos::hlt_loop();
}

entry_point!(kernel_main);

#[allow(unreachable_code)]
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    #[cfg(test)]
    {
        let _ = boot_info;
        test_main();
        skyos::hlt_loop();
    }

    run(boot_info);
    skyos::hlt_loop();
}
#[panic_handler]
#[allow(unreachable_code)]
fn __panic_handler(info: &PanicInfo) -> ! {
    #[cfg(test)]
    skyos::test_panic_handler(info);

    panic_handler(info);
}
