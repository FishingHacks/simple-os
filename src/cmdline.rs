use core::fmt::Display;

use alloc::{string::String, vec::Vec};
use lazy_static::lazy_static;
use pc_keyboard::DecodedKey;
use spin::Mutex;
use x86_64::instructions::interrupts::without_interrupts;

use crate::{print, println, serial_println, vga_buffer::WRITER};

type CmdResult = Result<(), Error>;
type Cmd = &'static dyn Fn(Vec<&str>) -> CmdResult;

lazy_static! {
    pub static ref CMD_LINE: Mutex<CommandLine> = Mutex::new(CommandLine::new());
}
const COMMANDS: &[(&'static str, &dyn Fn(Vec<&str>) -> CmdResult)] = &[
    ("echo", &echo),
    ("clear", &clear),
    ("cls", &clear),
];

fn echo(args: Vec<&str>) -> CmdResult {
    println!("{}", args.join(" "));

    Ok(())
}

fn clear(_: Vec<&str>) -> CmdResult {
    without_interrupts(|| WRITER.lock().clear_screen());

    Ok(())
}

pub enum Error {
    StrSlice(&'static str),
    Str(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Str(s) => f.write_str(s),
            Self::StrSlice(s) => f.write_str(s),
        }
    }
}

pub struct CommandLine {
    buffer: String,
}

impl CommandLine {
    fn new() -> Self {
        Self { buffer: String::with_capacity(100) }
    }

    pub fn init(&self) {
        print!("$ ");
    }

    pub fn process_key(&mut self, key: DecodedKey) {
        match key {
            DecodedKey::RawKey(k) => serial_println!("{:?}", k),
            DecodedKey::Unicode(char) => {
                match char {
                    char @ ('\x20'..='\x7e') => {
                        print!("{}", char);
                        self.buffer.push(char);
                    },
                    '\n' => {
                        print!("\n");
                        self.process_cmd();
                    }
                    '\x08' => {
                        if self.buffer.len() > 0 {
			    print!("\x08");
                            self.buffer.pop();
			}
                    },
                    
                    _ => {}
                }
            }
        }
    }

    fn process_cmd(&mut self) {
        let mut args = self.buffer.split(' ');
        if let Some(cmd) = args.next() {
            if let Some(func) = find_cmd(cmd) {
                let args: Vec<&str> = args.collect();

                if let Err(e) = func(args) {
                    println!("Failed to run {cmd}:\n{}", e);
                }
            } else {
                println!("Could not find command {cmd}");
            }
        }
        self.buffer.clear();
        self.init();
    }
}

fn find_cmd(cmd: &str) -> Option<Cmd> {
    for (name, func) in COMMANDS {
        if *name == cmd {
            return Some(func);
        }
    }

    None
}
