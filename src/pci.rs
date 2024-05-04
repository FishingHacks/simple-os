use alloc::{format, string::String, vec::Vec};
use x86_64::instructions::port::Port;

use crate::{
    drivers::{on_plug, PhysicalDevice}, mem::PAGE_SIZE, println
};

pub const CONFIG_ADDRESS: u16 = 0xCF8;
pub const CONFIG_DATA: u16 = 0xCFC;

macro_rules! get_pci_addr {
    ($bus: expr, $slot: expr, $func: expr, $offset: expr) => {
        (($bus as u32) << 16)
            | (($slot as u32) << 11)
            | (($func as u32) << 8)
            | (($offset & 0xfc) as u32)
            | 0x80000000
    };
}

pub fn read_u16(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    let addr = get_pci_addr!(bus, slot, func, offset * 2);
    unsafe { Port::new(CONFIG_ADDRESS).write(addr) };
    let data: u32 = unsafe { Port::new(CONFIG_DATA).read() };
    (data >> ((offset & 2) * 8) & 0xffff) as u16
}

pub fn read_u32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let addr = get_pci_addr!(bus, slot, func, offset * 4);
    unsafe { Port::new(CONFIG_ADDRESS).write(addr) };
    unsafe { Port::new(CONFIG_DATA).read() }
}

pub fn write_u16(bus: u8, slot: u8, func: u8, offset: u8, data: u16) {
    let addr = get_pci_addr!(bus, slot, func, offset * 2);
    unsafe {
        Port::new(CONFIG_ADDRESS).write(addr);
        Port::new(CONFIG_DATA).write(data);
    };
}

pub fn write_u32(bus: u8, slot: u8, func: u8, offset: u8, data: u32) {
    let addr = get_pci_addr!(bus, slot, func, offset * 4);
    unsafe {
        Port::new(CONFIG_ADDRESS).write(addr);
        Port::new(CONFIG_DATA).write(data)
    };
}

/// Reads PCI configuration and writes it into `buf`.
///
/// Arguments:
/// - `bus` is the bus number.
/// - `device` is the device number.
/// - `func` is the function number.
/// - `off` is the register offset.
/// - `buf` is the data buffer to write to.
fn read_data(bus: u8, device: u8, func: u8, off: usize, buf: &mut [u32]) {
    let end = 0x12.min(off + buf.len());

    for (buf_off, reg_off) in (off..end).enumerate() {
        buf[buf_off] = read_u32(bus, device, func, reg_off as _);
    }
}

/// Writes PCI configuration from `buf`.
///
/// Arguments:
/// - `bus` is the bus number.
/// - `device` is the device number.
/// - `func` is the function number.
/// - `off` is the register offset.
/// - `buf` is the data buffer to read from.
fn write_data(bus: u8, device: u8, func: u8, off: usize, buf: &[u32]) {
    let end = 16.min(off + buf.len());

    for (buf_off, reg_off) in (off..end).enumerate() {
        write_u32(bus, device, func, reg_off as _, buf[buf_off]);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BARType {
    /// The base register is 32 bits wide.
    Size32,
    /// The base register is 64 bits wide.
    Size64,
}

#[derive(Debug)]
pub enum BAR {
    IOSpace {
        /// Address to the registers in I/O space.
        address: u64,

        /// The size of the address space in bytes.
        size: usize,
    },

    MemorySpace {
        typ: BARType,

        prefetchable: bool,
        address: u64,
        size: usize,
    },
}

impl BAR {
    /// Returns the base address.
    pub fn get_address(&self) -> *mut () {
        match self {
            Self::MemorySpace { address, .. } => *address as _,

            Self::IOSpace { address, .. } => *address as _,
        }
    }

    /// Returns the amount of memory.
    pub fn get_size(&self) -> usize {
        match self {
            Self::MemorySpace { size, .. } => *size,

            Self::IOSpace { size, .. } => *size,
        }
    }

    /// Tells whether the memory is prefetchable.
    pub fn is_prefetchable(&self) -> bool {
        match self {
            Self::MemorySpace { prefetchable, .. } => *prefetchable,

            Self::IOSpace { .. } => false,
        }
    }
}

#[derive(Debug)]
pub struct PCIDevice {
    bus: u8,
    device: u8,
    function: u8,

    /// the device id
    device_id: u16,
    /// the vendor id
    vendor_id: u16,
    /// the status register
    status: u16,
    /// the command register
    command: u16,

    /// The device's class code, telling what the device is.
    class: u8,
    /// The device's subclass code, giving more informations on the device.
    subclass: u8,
    /// Value giving more informations on the device's compatibilities.
    prog_if: u8,
    /// The device's revision ID.
    revision_id: u8,

    /// Built-In Self Test status.
    bist: u8,
    /// Defines the header type of the device, to determine what informations
    /// follow.
    header_type: u8,
    /// Specifies the latency timer in units of PCI bus clocks.
    latency_timer: u8,
    /// Specifies the system cache line size in 32-bit units.
    cache_line_size: u8,

    /// Additional informations about the device.
    info: [u32; 12],

    /// The list of BARs for the device.
    bars: Vec<Option<BAR>>,

    unique_identifier: String,
}

impl PCIDevice {
    fn get_max_bars_count(&self) -> u8 {
        match self.header_type {
            0x00 => 6,
            0x01 => 2,

            _ => 0,
        }
    }

    fn get_bar_reg_off(&self, n: u8) -> Option<u16> {
        if n < self.get_max_bars_count() {
            Some(0x4 + n as u16)
        } else {
            None
        }
    }

    /// Returns the size of the address space of the `n`th BAR.
    ///
    /// `io` tells whether the BAR is in I/O space.
    fn get_bar_size(&self, n: u8, io: bool) -> Option<usize> {
        let reg_off = self.get_bar_reg_off(n)?;
        // Saving the register
        let save = read_u32(self.bus, self.device, self.function, reg_off as _);

        // Writing all 1s on register
        write_u32(self.bus, self.device, self.function, reg_off as _, !0u32);

        let mut size =
            (!read_u32(self.bus, self.device, self.function, reg_off as _)).wrapping_add(1);
        if io {
            size &= 0xffff;
        }

        // Restoring the register's value
        write_u32(self.bus, self.device, self.function, reg_off as _, save);

        Some(size as _)
    }

    pub fn load_bar(&self, n: u8) -> Option<BAR> {
        let Some(off) = self.get_bar_reg_off(n) else {
            return None;
        };

        let value = read_u32(self.bus, self.device, self.function, off as _);
        let io = (value & 0b11) != 0;
        let size = self.get_bar_size(n, io)?;

        if !io {
            let prefetchable = (value & 0b1000) != 0;
            let typ = match (value >> 1) & 0b11 {
                0x0 => BARType::Size32,
                0x2 => BARType::Size64,
                _ => return None,
            };
            let mut address = match typ {
                BARType::Size32 => (value & 0xfffffff0) as u64,

                BARType::Size64 => {
                    let Some(next_bar_off) = self.get_bar_reg_off(n + 1) else {
                        return None;
                    };

                    // The next BAR's value
                    let next_value =
                        read_u32(self.bus, self.device, self.function, next_bar_off as _);
                    (value & 0xfffffff0) as u64 | ((next_value as u64) << 32)
                }
            };
            if address == 0 {
                return None;
            }

            let pages = size.div_ceil(PAGE_SIZE);

            println!("");
            return None;
            panic!("shitty mmap and munmap :help:");
        }

        let address = (value as u64) & 0xfffffffc;
        if address == 0 {
            return None;
        }

        Some(BAR::IOSpace {
            address,
            size,
        })
    }

    pub fn new(bus: u8, device: u8, function: u8, data: &[u32; 16]) -> Self {
        let mut dev = Self {
            bus,
            device,
            function,

            vendor_id: (data[0] & 0xffff) as _,
            device_id: ((data[0] >> 16) & 0xffff) as _,

            command: (data[1] & 0xffff) as _,
            status: ((data[1] >> 16) & 0xffff) as _,

            class: ((data[2] >> 24) & 0xff) as _,
            subclass: ((data[2] >> 16) & 0xff) as _,
            prog_if: ((data[2] >> 8) & 0xff) as _,
            revision_id: (data[2] & 0xff) as _,

            bist: ((data[3] >> 24) & 0xff) as _,
            header_type: ((data[3] >> 16) & 0xff) as _,
            latency_timer: ((data[3] >> 8) & 0xff) as _,
            cache_line_size: (data[3] & 0xff) as _,

            info: [
                data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11], data[12],
                data[13], data[14], data[15],
            ],

            bars: Vec::new(),

            unique_identifier: format!("enp{}s{}f{}", bus, device, function),
        };

        let mut i = 0;
        while i < dev.get_max_bars_count() {
            dev.bars.push(dev.load_bar(i));

            i += 1;
        }

        dev
    }

    /// Returns the PCI bus ID.
    #[inline(always)]
    pub fn get_bus(&self) -> u8 {
        self.bus
    }

    /// Returns the PCI device ID.
    #[inline(always)]
    pub fn get_device(&self) -> u8 {
        self.device
    }

    /// Returns the PCI function ID.
    #[inline(always)]
    pub fn get_function(&self) -> u8 {
        self.function
    }

    /// Returns the header type of the device.
    #[inline(always)]
    pub fn get_header_type(&self) -> u8 {
        // Clear the Multi-Function flag
        self.header_type & 0b01111111
    }
}

impl PhysicalDevice for PCIDevice {
    fn get_device_id(&self) -> u16 {
        self.device_id
    }

    fn get_vendor_id(&self) -> u16 {
        self.vendor_id
    }

    fn get_command_reg(&self) -> Option<u16> {
        Some(self.command)
    }

    fn get_status_reg(&self) -> Option<u16> {
        Some(self.status)
    }

    fn get_class(&self) -> u16 {
        self.class as _
    }

    fn get_subclass(&self) -> u16 {
        self.subclass as _
    }

    fn get_prog_if(&self) -> u8 {
        self.prog_if
    }

    fn is_hotplug(&self) -> bool {
        false
    }

    fn get_bars(&self) -> &[Option<BAR>] {
        &self.bars
    }

    fn get_interrupt_line(&self) -> Option<u8> {
        let n = (self.info[11] & 0xff) as u8;

        if n != 0xff {
            Some(n)
        } else {
            None
        }
    }

    fn get_interrupt_pin(&self) -> Option<u8> {
        let n = ((self.info[11] >> 8) & 0xff) as u8;

        if n != 0 {
            Some(n)
        } else {
            None
        }
    }

    fn unique_identifier(&self) -> &str {
        &self.unique_identifier
    }
}

/// This manager handles every devices connected to the PCI bus.
///
/// Since the PCI bus is not a hotplug bus, calling `on_unplug` on this structure has no effect.
pub struct PCIManager {
    /// The list of PCI devices.
    devices: Vec<PCIDevice>,
}

impl PCIManager {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Scans for PCI devices and registers them on the manager.
    ///
    /// If the PCI has already been scanned, this function does nothing.
    pub fn scan(&mut self) {
        // Avoid calling `on_plug` twice for the same devices
        if !self.devices.is_empty() {
            return;
        }

        for bus in 0..=255 {
            for device in 0..32 {
                let vendor_id = read_u32(bus, device, 0, 0) & 0xffff;
                // If the device doesn't exist, ignore
                if vendor_id == 0xffff {
                    continue;
                }

                // Reading device's PCI data
                let mut data: [u32; 16] = [0; 16];
                read_data(bus, device, 0, 0, &mut data);

                let header_type = ((data[3] >> 16) & 0xff) as u8;
                let max_functions_count = {
                    if header_type & 0x80 != 0 {
                        // Multi-function device
                        8
                    } else {
                        // Single-function device
                        1
                    }
                };

                // Iterating on every functions of the device
                for func in 0..max_functions_count {
                    let vendor_id = read_u32(bus, device, func, 0) & 0xffff;
                    // If the function doesn't exist, ignore
                    if vendor_id == 0xffff {
                        continue;
                    }

                    // Reading function's PCI data
                    read_data(bus, device, func, 0, &mut data);

                    // Enabling I/O space for BARs
                    data[1] |= 0b1;
                    write_u32(bus, device, func, 0x1, data[1]);

                    let dev = PCIDevice::new(bus, device, func, &data);
                    if dev.header_type != 0 {
                        println!(
                            "Found enp{}s{}f{} ht: {:x} ({:x} {:x}; {:x}); skipping",
                            dev.bus,
                            dev.device,
                            dev.function,
                            dev.header_type,
                            dev.class,
                            dev.subclass,
                            dev.prog_if
                        );
                        continue;
                    }

                    println!(
                        "Found enp{}s{}f{} {:x} {:x} ({:x} {:x}; {:x})",
                        dev.bus,
                        dev.device,
                        dev.function,
                        dev.vendor_id,
                        dev.device_id,
                        dev.class,
                        dev.subclass,
                        dev.prog_if
                    );
                    on_plug(&dev);
                    self.devices.push(dev);
                }
            }
        }
    }

    /// Returns the list of PCI devices.
    ///
    /// If the PCI hasn't been scanned, the function returns an empty vector.
    #[inline(always)]
    pub fn get_devices(&self) -> &Vec<PCIDevice> {
        &self.devices
    }
}
