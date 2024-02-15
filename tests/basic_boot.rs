#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(skyos::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use bootloader::{BootInfo, entry_point};
use skyos::println;

entry_point!(run);

fn run(_boot_info: &'static BootInfo) -> ! {
    test_main();

    skyos::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    skyos::test_panic_handler(info)
}

#[test_case]
fn test_println() {
    println!("test_println output");
}
