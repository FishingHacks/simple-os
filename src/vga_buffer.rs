use core::fmt::{Arguments, Result, Write};

use core::iter::Iterator;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;
use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    fn new(fg: Color, bg: Color) -> Self {
        Self((bg as u8) << 4 | fg as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color: ColorCode,
}

pub const BUFFER_HEIGHT: usize = 25;
pub const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    column_pos: usize,
    cur_color: ColorCode,
    buffer: &'static mut Buffer,
    row_pos: usize,
}

impl Writer {
    fn clear(&mut self) {
        let blank = ColorCode::new(Color::White, Color::Black);
        for row in 0..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                self.buffer.chars[row][col].write(ScreenChar { ascii_character: b' ', color: blank });
            }
        }
    }

    pub fn clear_screen(&mut self) {
        self.column_pos = 0;
        self.row_pos = 0;
        self.clear();
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\t' => self.column_pos += 4,
            0x8 /* \b (backspace) */ => {
                if self.column_pos > 0 {
                    self.column_pos -= 1;
                    self.buffer.chars[self.row_pos][self.column_pos].write(ScreenChar { ascii_character: b' ', color: self.cur_color });
                }
            }
            byte => {
                if self.column_pos >= BUFFER_WIDTH {
                    self.new_line();
                }
                
                let row = self.row_pos;
                let col = self.column_pos;
                
                let color = self.cur_color;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color,
                });
                self.column_pos += 1;
            }
        }
        set_cursor(self.column_pos, self.row_pos);
    }

    fn new_line(&mut self) {
        if self.row_pos < BUFFER_HEIGHT - 1 {
            self.column_pos = 0;
            self.row_pos += 1;
            return;
        }

        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_line(BUFFER_HEIGHT - 1);
        self.column_pos = 0;
    }

    fn clear_line(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color: self.cur_color,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }

    pub fn write_str(&mut self, str: &str) {
        for byte in str.chars().map(|char| transform_char(char)) {
            self.write_byte(byte);
        }
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new({
        let mut writer = Writer {
            column_pos: 0,
            row_pos: 0,
            cur_color: ColorCode::new(Color::White, Color::Black),
            buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
        };
        writer.clear();
        writer
    });
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> Result {
        self.write_str(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! println {
    () => ($crate::vga_buffer::_print(format_args!("\n")));
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!("{}\n", format_args!($($arg)*))));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    interrupts::without_interrupts(|| WRITER.lock().write_fmt(args).unwrap());
}

pub fn set_color(new_color: ColorCode) {
    interrupts::without_interrupts(|| WRITER.lock().cur_color = new_color);
}

pub fn set_fg(new_color: Color) {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();

        writer.cur_color = ColorCode(writer.cur_color.0 & 0xf0 | new_color as u8);
    });
}

pub fn set_bg(new_color: Color) {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();

        writer.cur_color = ColorCode((new_color as u8) << 4 | writer.cur_color.0 & 0xf);
    });
}

pub fn disable_cursor() {
    unsafe {
        Port::new(0x3d4).write(0x0a_u8);
        Port::new(0x3d5).write(0x20_u8);
    }
}

pub fn enable_cursor() {
    unsafe {
        let mut porta = Port::new(0x3d4);
        let mut portb = Port::new(0x3d5);

        porta.write(0x0a_u8);
        let val = portb.read() as u8 & 0xc0;
        portb.write(val | 0xf);

        porta.write(0x0b_u8);
        let val = portb.read() as u8 & 0xe0;
        portb.write(val | 0xf);
    }
}

pub fn set_cursor(x: usize, y: usize) {
    // if x >= BUFFER_WIDTH || y >= BUFFER_HEIGHT {
    //     return disable_cursor();
    // }

    let mut porta = Port::new(0x3d4);
    let mut portb = Port::new(0x3d5);
    let pos = y * BUFFER_WIDTH + x;

    unsafe {
        porta.write(0x0f_u8);
        portb.write((pos & 0xff) as u8);
        porta.write(0x0e_u8);
        portb.write(((pos >> 8) & 0xff) as u8);
    }
}

pub fn transform_char(char: char) -> u8 {
    match char {
        '\n' | '\t'| '\x08' | ('\x20'..='\x7e') => char as u8,
        '•' => 0x07,
        '░' => 0xb0,
        '▒' => 0xb1,
        '▓' => 0xb2,
        '│' => 0xb3,
        '┤' => 0xb4,
        '╡' => 0xb5,
        '╢' => 0xb6,
        '╖' => 0xb7,
        '╕' => 0xb8,
        '╣' => 0xb9,
        '║' => 0xba,
        '╗' => 0xbb,
        '╝' => 0xbc,
        '╜' => 0xbd,
        '╛' => 0xbe,
        '┐' => 0xbf,
        '└' => 0xc0,
        '┴' => 0xc1,
        '┬' => 0xc2,
        '├' => 0xc3,
        '─' => 0xc4,
        '┼' => 0xc5,
        '╞' => 0xc6,
        '╟' => 0xc7,
        '╚' => 0xc8,
        '╔' => 0xc9,
        '╩' => 0xca,
        '╦' => 0xcb,
        '╠' => 0xcc,
        '═' => 0xcd,
        '╬' => 0xce,
        '╧' => 0xcf,
        '╨' => 0xd0,
        '╤' => 0xd1,
        '╥' => 0xd2,
        '╙' => 0xd3,
        '╘' => 0xd4,
        '╒' => 0xd5,
        '╓' => 0xd6,
        '╫' => 0xd7,
        '╪' => 0xd8,
        '┘' => 0xd9,
        '┌' => 0xda,
        '█' => 0xdb,
        '▄' => 0xdc,
        '▌' => 0xdd,
        '▐' => 0xde,
        '▀' => 0xdf,
        '■' => 0xfe,
        _ => 0xfe,
    }
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
