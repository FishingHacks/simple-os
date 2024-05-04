use alloc::{boxed::Box, vec::Vec};
use spin::Mutex;

use crate::pci::BAR;
mod ahci_driver;

pub trait PhysicalDevice {
    fn get_device_id(&self) -> u16;
    fn get_vendor_id(&self) -> u16;
    fn get_command_reg(&self) -> Option<u16>;
    fn get_status_reg(&self) -> Option<u16>;
    fn get_class(&self) -> u16;
    fn get_subclass(&self) -> u16;
    fn get_prog_if(&self) -> u8;
    fn is_hotplug(&self) -> bool;
    fn get_bars(&self) -> &[Option<BAR>];
    fn get_interrupt_line(&self) -> Option<u8>;
    fn get_interrupt_pin(&self) -> Option<u8>;
    fn unique_identifier(&self) -> &str;
}

pub trait DriverManager: Send + Sync {
    fn on_plug(&self, dev: &dyn PhysicalDevice) -> Option<Box<dyn Driver>>;
}

pub trait Driver: Send + Sync {
    fn get_name(&self) -> &str;
    fn on_unplug(&self, dev: &dyn PhysicalDevice) -> bool;
}

static DRIVER_MANAGERS: Mutex<Vec<Box<dyn DriverManager>>> = Mutex::new(Vec::new());
static DRIVERS: Mutex<Vec<Box<dyn Driver>>> = Mutex::new(Vec::new());

pub fn on_plug(dev: &dyn PhysicalDevice) {
    let driver_managers = DRIVER_MANAGERS.lock();
    let mut drivers = DRIVERS.lock();

    for i in 0..driver_managers.len() {
        if let Some(driver) = driver_managers[i].on_plug(dev) {
            drivers.push(driver);
        }
    }
}

pub fn on_unplug(dev: &dyn PhysicalDevice) {
    let mut drivers = DRIVERS.lock();

    let max = drivers.len();
    for i in 1..=max {
        let i = max - i;
        if drivers[i].on_unplug(dev) {
            drivers.remove(i);
        }
    }
}
