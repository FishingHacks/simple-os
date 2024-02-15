use x86_64::{instructions::port::Port as x86_port, structures::port::PortWrite};

#[derive(Debug, Clone, Copy)]
pub enum Port {
    Primary,
    PrimaryControl,
    Secondary,
    SecondaryControl,
}

impl Port {
    pub fn get_addr(&self) -> u16 {
        match self {
            Self::Primary => 0x1f0,
            Self::Secondary => 0x170,
            Self::PrimaryControl => 0x3f6,
            Self::SecondaryControl => 0x376,
        }
    }

    pub fn get_addr_end(&self) -> u16 {
        match self {
            Self::Primary => 0x1f7,
            Self::Secondary => 0x177,
            Self::PrimaryControl => 0x3f7,
            Self::SecondaryControl => 0x377,
        }
    }

    pub fn port(&self) -> x86_port<u16> {
        x86_port::new(self.get_addr())
    }

    pub fn port_offset(&self, offset: u16) -> x86_port<u16> {
        x86_port::new(self.get_addr_with_offset(offset))
    }

    pub fn get_addr_with_offset(&self, offset: u16) -> u16 {
        if offset <= self.get_addr_end() {
            self.get_addr() + offset
        } else {
            panic!("Tried to access port {offset}");
        }
    }

    pub fn is_ctrl_port(&self) -> bool {
        match self {
            Self::Primary | Self::Secondary => false,
            Self::PrimaryControl | Self::SecondaryControl => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Register {
    Data = 0,
    ErrorFeatures = 1,
    SectorCount = 2,
    SectorNumberOrLBAlow = 3,
    CylinderLowOrLBAmid = 4,
    CylinderHighOrLBAhigh = 5,
    DriveHead = 6,
    StatusCommand = 7,
}

impl Register {
    pub fn get_port(&self, port: Port) -> x86_port<u16> {
        if port.is_ctrl_port() {
            panic!("Tried to access a register on a control port")
        }

        port.port_offset(*self as u16)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ControlRegister {
    AltStatusDeviceControl = 0,
    DriveAddr = 1,
}

impl ControlRegister {
    pub fn get_port(&self, port: Port) -> x86_port<u16> {
        if !port.is_ctrl_port() {
            panic!("Tried to access a control register on a normal port")
        }

        port.port_offset(*self as u16)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorRegisterBits {
    AddrMarkNotFound = 0,
    TrackZeroNotFound = 1,
    AbortedCommand = 2,
    MediaChangeRequest = 3,
    IDNotFound = 4,
    MediaChanged = 5,
    UncorrectableDataError = 6,
    BadBlockDetected = 7,
}

impl ErrorRegisterBits {
    pub fn is_set_raw(&self, value: u8) -> bool {
        value & (1 << *self as u8) > 0
    }

    pub fn is_set(&self, port: Port) -> bool {
        (unsafe { Register::ErrorFeatures.get_port(port).read() as u8 } & (1 << *self as u8)) > 0
    }
}

pub struct DriveHeadRegister;

impl DriveHeadRegister {
    pub fn read_raw(port: Port) -> u8 {
        unsafe { Register::DriveHead.get_port(port).read() as u8 }
    }

    pub fn get_drv_num(port: Port) -> bool {
        (Self::read_raw(port) >> 4) > 0
    }

    pub fn uses_lba(port: Port) -> bool {
        (Self::read_raw(port) & 64) > 0 // the 7th bit (bit 6, 1 << 6) set?
    }

    pub fn bit0to3(port: Port) -> u8 {
        Self::read_raw(port) & 0b1111
    }

    pub fn bit24to27(port: Port) -> u8 {
        Self::read_raw(port) & 0b1111
    }

    pub fn write_value(port: Port, drv_num: bool, with_lba: bool, chs_or_lba_bits: u8) {
        let mut value: u8 = 0b10100000; // bits 5 and 7 set
        if drv_num {
            value |= 1 << 4;
        }
        if with_lba {
            value |= 1 << 6;
        }
        value |= chs_or_lba_bits & 64;
        unsafe { Register::DriveHead.get_port(port).write(value as u16) }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StatusRegisterBits {
    // Indicates an error occurred. Send a new command to clear it (or nuke it with a Software Reset).
    Error = 0,
    /// always set to 0
    Idx = 1,
    /// Corrected data. Always set to zero.
    CorrectedData = 2,
    /// Set when the drive has PIO data to transfer, or is ready to accept PIO data.
    HasData = 3,
    /// Overlapped Mode Service Request.
    OverlappedModeServiceRequest = 4,
    /// Drive Fault Error (does not set ERR).
    DriveFault = 5,
    /// Bit is clear when drive is spun down, or after an error. Set otherwise.
    Ready = 6,
    /// Indicates the drive is preparing to send/receive data (wait for it to clear). In case of 'hang' (it never clears), do a software reset.
    Busy = 7,
}

impl StatusRegisterBits {
    pub fn is_set_raw(&self, value: u8) -> bool {
        value & (1 << *self as u8) > 0
    }

    pub fn is_set(&self, port: Port) -> bool {
        (unsafe { Register::ErrorFeatures.get_port(port).read() as u8 } & (1 << *self as u8)) > 0
    }
}

pub struct DeviceControRegister;

impl DeviceControRegister {
    pub fn set(port: Port, stop_interrupts: bool, read_high_bit: bool) {
        let mut value: u8 = 0;
        if stop_interrupts {
            value |= 0b01000000;
        }
        if read_high_bit {
            value |= 0b1000000;
        }
        unsafe {
            ControlRegister::AltStatusDeviceControl
                .get_port(port)
                .write(value as u16)
        }
    }

    pub fn reset_bus(port: Port, stop_interrupts: bool, read_high_bit: bool) {
        let mut native_port = ControlRegister::AltStatusDeviceControl.get_port(port);

        unsafe {
            native_port.write(0b001);
        } // writes bit 2 (3rd bit)
          // wait 5 microseconds
        wait_ns(5000);
        unsafe {
            native_port.write(0);
        }
        Self::set(port, stop_interrupts, read_high_bit);
    }
}

fn wait_ns(mut nanoseconds: u64) {
    nanoseconds /= 30;
    let mut i = 0;
    while i < nanoseconds {
        unsafe {
            PortWrite::write_to_port(0x80, 0 as u8);
        }
        i += 1;
    }
}

pub struct DriveAddrCtrlReg;

impl DriveAddrCtrlReg {
    pub fn is_drive_one_selected(port: Port) -> bool {
        unsafe { ControlRegister::DriveAddr.get_port(port).read() & 0b10 > 0 }
    }

    pub fn is_drive_zero_selected(port: Port) -> bool {
        unsafe { ControlRegister::DriveAddr.get_port(port).read() & 0b1 > 0 }
    }

    pub fn cur_sel_head(port: Port) -> u8 {
        unsafe { ControlRegister::DriveAddr.get_port(port).read() as u8 >> 2 & 0b1111 }
    }

    pub fn is_writing(port: Port) -> bool {
        unsafe { ControlRegister::DriveAddr.get_port(port).read() & 0b01000000 == 0 }
    }
}

// next: IRQ