use crate::pci::BAR;

use super::DriverManager;

pub struct AhciDriverManager;

impl DriverManager for AhciDriverManager {
    fn on_plug(
        &self,
        dev: &dyn super::PhysicalDevice,
    ) -> Option<alloc::boxed::Box<dyn super::Driver>> {
        if dev.get_class() == 0x1 && dev.get_subclass() == 0x6 && dev.get_prog_if() == 0x1 {
            // AHCI Device
        }

        None
    }
}

pub struct AhciDriver {
    bars: [Option<BAR>; 1],
}
