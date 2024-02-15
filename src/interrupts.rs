use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::{
    instructions::{interrupts::without_interrupts, port::Port},
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

use crate::{cmdline::CMD_LINE, gdt, print, println};

macro_rules! handler {
    ($name: tt) => {
        extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            println!(
                concat!("EXCEPTION: ", stringify!($name), ": {:?}"),
                stack_frame
            )
        }
    };
    ($name: tt, $other: expr) => {
        extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame, value: u64) {
            println!(
                concat!("EXCEPTION: ", stringify!($name), ": {:?}; {}"),
                stack_frame, value
            )
        }
    };
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);

        idt.debug.set_handler_fn(debug);
        idt.non_maskable_interrupt
            .set_handler_fn(non_maskable_interrupt);
        idt.overflow.set_handler_fn(overflow);
        idt.bound_range_exceeded
            .set_handler_fn(bound_range_exceeded);
        idt.invalid_opcode.set_handler_fn(invalid_opcode);
        idt.device_not_available
            .set_handler_fn(device_not_available);
        idt.x87_floating_point.set_handler_fn(x87_floating_point);
        idt.simd_floating_point.set_handler_fn(simd_floating_point);
        idt.virtualization.set_handler_fn(virtualization);
        idt.hv_injection_exception
            .set_handler_fn(hv_injection_exception);
        idt.invalid_tss.set_handler_fn(invalid_tss);
        idt.segment_not_present.set_handler_fn(segment_not_present);
        idt.stack_segment_fault.set_handler_fn(stack_segment_fault);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault);
        idt.alignment_check.set_handler_fn(alignment_check);
        idt.cp_protection_exception
            .set_handler_fn(cp_protection_exception);
        idt.vmm_communication_exception
            .set_handler_fn(vmm_communication_exception);
        idt.security_exception.set_handler_fn(security_exception);
        idt.divide_error.set_handler_fn(divide_error);

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

handler!(divide_error);
handler!(debug);
handler!(non_maskable_interrupt);
handler!(overflow);
handler!(bound_range_exceeded);
handler!(invalid_opcode);
handler!(device_not_available);
handler!(x87_floating_point);
handler!(simd_floating_point);
handler!(virtualization);
handler!(hv_injection_exception);
handler!(invalid_tss, ());
handler!(segment_not_present, ());
handler!(stack_segment_fault, ());
handler!(general_protection_fault, ());
handler!(alignment_check, ());
handler!(cp_protection_exception, ());
handler!(vmm_communication_exception, ());
handler!(security_exception, ());

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    crate::hlt_loop();
}

#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}

// ╔═══════════════════════════════════════════╗
// ║                                           ║
// ║   H A R D W A R E   I N T E R R U P T S   ║
// ║                                           ║
// ╚═══════════════════════════════════════════╝

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
            Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore)
        );
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            without_interrupts(|| CMD_LINE.lock().process_key(key));
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}
