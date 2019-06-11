extern crate rand;

use rand::Rng;
use std::fs::File;
use std::io::Read;

const RAM_SIZE: usize = 0x1000; // 4096
const NUM_REGISTERS: usize = 16;
const NUM_PIXELS: usize = 64 * 32;
const STACK_SIZE: usize = 16;
const NUM_KEYS: usize = 16;
const START_PC: usize = 0x200; // 512

// Holds caracters 0 to F
// Each character is 5 bytes
const FONT_SET: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80, 0xF0, 0xF0,
    0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0, 0x10, 0xF0, 0xF0, 0x80,
    0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90, 0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0,
    0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0, 0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80,
    0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0, 0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
];

pub struct Simulator {
    ram: [u8; RAM_SIZE],

    v: [u8; NUM_REGISTERS], // registers
    i: u16,                 // index register
    pc: u16,                // program counter

    stack: [u16; STACK_SIZE],
    sp: u16, // stack pointer

    gfx: [u8; NUM_PIXELS], // graphical memory (the screen)
    gfx_changed: bool,

    delay_timer: u8,
    sound_timer: u8,

    key: [bool; NUM_KEYS], // keypad
}

impl Simulator {
    pub fn new() -> Self {
        let mut ram = [0u8; RAM_SIZE];

        // load font set
        for i in 0..FONT_SET.len() {
            ram[i] = FONT_SET[i];
        }

        Simulator {
            ram: ram,
            v: [0; NUM_REGISTERS],
            i: 0,
            pc: START_PC as u16,
            stack: [0; STACK_SIZE],
            sp: 0,
            gfx: [0; NUM_PIXELS],
            gfx_changed: false,
            delay_timer: 0,
            sound_timer: 0,
            key: [false; NUM_KEYS],
        }
    }

    pub fn load_program(&mut self, program: &[u8]) {
        for (i, &byte) in program.iter().enumerate() {
            if START_PC + i < RAM_SIZE {
                self.ram[START_PC + i] = byte;
            } else {
                break;
            }
        }
    }

    fn get_opcode(&mut self) -> u16 {
        let pc = self.pc as usize;
        // opcodes are 2 bytes, so we need to grab two ram locations
        // and combine them.
        let high = (self.ram[pc] as u16) << 8;
        let low = self.ram[pc + 1] as u16;
        high | low
    }

    fn next_pc(&mut self) -> u16 {
        self.pc += 2;
        self.pc
    }

    pub fn step(&mut self, opcode: u16) {
        let opcode_nibbles = (
            (opcode & 0xF000) >> 12 as u8,
            (opcode & 0x0F00) >> 8 as u8,
            (opcode & 0x00F0) >> 4 as u8,
            (opcode & 0x000F) as u8,
        );

        let nnn = opcode & 0x0FFF;
        let nn = (opcode & 0x00FF) as u8;

        self.pc = match opcode_nibbles {
            (0x00, 0x00, 0x0E, 0x00) => self.op_00E0(),
            (0x00, 0x00, 0x0E, 0x0E) => self.op_00EE(),
            (0x01, _, _, _) => self.op_1NNN(nnn),
            (0x02, _, _, _) => self.op_2NNN(nnn),
            (0x03, x, _, _) => self.op_3XNN(x as usize, nn),
            (0x04, x, _, _) => self.op_4XNN(x as usize, nn),
            (0x05, x, y, 0x00) => self.op_5XY0(x as usize, y as usize),
            (0x06, x, _, _) => self.op_6XNN(x as usize, nn),
            (0x07, x, _, _) => self.op_7XNN(x as usize, nn),
            (0x08, x, y, 0x00) => self.op_8XY0(x as usize, y as usize),
            (0x08, x, y, 0x01) => self.op_8XY1(x as usize, y as usize),
            (0x08, x, y, 0x02) => self.op_8XY2(x as usize, y as usize),
            (0x08, x, y, 0x03) => self.op_8XY3(x as usize, y as usize),
            (0x08, x, y, 0x04) => self.op_8XY4(x as usize, y as usize),
            (0x08, x, y, 0x05) => self.op_8XY5(x as usize, y as usize),
            (0x08, x, y, 0x06) => self.op_8XY6(x as usize, y as usize),
            (0x08, x, y, 0x07) => self.op_8XY7(x as usize, y as usize),
            (0x08, x, y, 0x0E) => self.op_8XYE(x as usize, y as usize),
            (0x09, x, y, 0x00) => self.op_9XY0(x as usize, y as usize),
            (0x0A, _, _, _) => self.op_ANNN(nnn),
            (0x0B, _, _, _) => self.op_BNNN(nnn),
            (0x0C, x, _, _) => self.op_CXNN(x as usize, nn),
            (0x0D, x, y, n) => self.op_DXYN(x as usize, y as usize, n as u8),
            (0x0E, x, 0x09, 0x0E) => self.op_EX9E(x as usize),
            (0x0E, x, 0x0A, 0x01) => self.op_EXA1(x as usize),
            (0x0F, x, 0x00, 0x07) => self.op_FX07(x as usize),
            (0x0F, x, 0x00, 0x0A) => self.op_FX0A(x as usize),
            (0x0F, x, 0x01, 0x05) => self.op_FX15(x as usize),
            (0x0F, x, 0x01, 0x08) => self.op_FX18(x as usize),
            (0x0F, x, 0x01, 0x0E) => self.op_FX1E(x as usize),
            (0x0F, x, 0x02, 0x09) => self.op_FX29(x as usize),
            (0x0F, x, 0x03, 0x03) => self.op_FX33(x as usize),
            (0x0F, x, 0x05, 0x05) => self.op_FX55(x as usize),
            (0x0F, x, 0x06, 0x05) => self.op_FX65(x as usize),
            _ => self.next_pc(), //TODO message that we couldn't find opcode
        }
    }

    // Clear the display
    fn op_00E0(&mut self) -> u16 {
        for i in 0..NUM_PIXELS {
            self.gfx[i] = 0
        }
        self.gfx_changed = true;
        self.next_pc()
    }

    // Return from subroutine
    fn op_00EE(&mut self) -> u16 {
        self.sp -= 1;
        self.pc = self.stack[self.sp as usize];
        self.next_pc()
    }

    // Jump to NNN
    fn op_1NNN(&mut self, nnn: u16) -> u16 {
        self.pc = nnn;
        self.pc
    }

    // Call subroutine NNN
    fn op_2NNN(&mut self, nnn: u16) -> u16 {
        self.sp += 1;
        self.stack[self.sp as usize] = self.pc;
        //TODO check for stack overflow?
        self.pc = nnn;
        self.pc
    }

    // Skip if VX == NN
    fn op_3XNN(&mut self, x: usize, nn: u8) -> u16 {
        if self.v[x] == nn {
            self.next_pc();
        }
        self.next_pc()
    }

    // Skip if VX != NN
    fn op_4XNN(&mut self, x: usize, nn: u8) -> u16 {
        if self.v[x] != nn {
            self.next_pc();
        }
        self.next_pc()
    }

    // Skip if VX == VY
    fn op_5XY0(&mut self, x: usize, y: usize) -> u16 {
        if self.v[x] == self.v[y] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Set VX to NN
    fn op_6XNN(&mut self, x: usize, nn: u8) -> u16 {
        self.v[x] = nn;
        self.next_pc()
    }

    // Add NN to VX
    fn op_7XNN(&mut self, x: usize, nn: u8) -> u16 {
        //TODO is this needed?
        // Do the addition as u16 then truncate to u8 so we don't roll over
        self.v[x] = ((self.v[x] as u16) + (nn as u16)) as u8;
        self.next_pc()
    }

    // Set VX to VY
    fn op_8XY0(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] = self.v[y];
        self.next_pc()
    }

    // Set VX to VX or VY
    fn op_8XY1(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] |= self.v[y];
        self.next_pc()
    }

    // Set VX to VX and VY
    fn op_8XY2(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] &= self.v[y];
        self.next_pc()
    }

    // Set VX to VX xor VY
    fn op_8XY3(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] ^= self.v[y];
        self.next_pc()
    }

    // Add VY to VX and use VF as carry bit
    // 1 for carry, 0 otherwise
    fn op_8XY4(&mut self, x: usize, y: usize) -> u16 {
        let sum = self.v[x] as u16 + self.v[y] as u16;
        self.v[x] = sum as u8;
        self.v[0x0F] = (sum >> 8) as u8;
        self.next_pc()
    }

    // Subtract VY from VX and use VF as borrow bit
    // 0 for borrow, 1 otherwise
    fn op_8XY5(&mut self, x: usize, y: usize) -> u16 {
        let borrow = self.v[y] > self.v[x];
        self.v[x] = self.v[x].wrapping_sub(self.v[y]);
        self.v[0x0F] = !borrow as u8;
        self.next_pc()
    }

    // Put least significant bit from VX in VF then shift VX right once
    fn op_8XY6(&mut self, x: usize, y: usize) -> u16 {
        self.v[0x0F] = self.v[x] & 0x01;
        self.v[x] >>= 1;
        self.next_pc()
    }

    // Set VX to VY - VX and use VF as borrow bit
    // 0 for borrow, 1 otherwise
    fn op_8XY7(&mut self, x: usize, y: usize) -> u16 {
        let borrow = self.v[x] > self.v[y];
        self.v[x] = self.v[y].wrapping_sub(self.v[x]);
        self.v[0x0F] = !borrow as u8;
        self.next_pc()
    }

    // Put most significant bit from VX in VF then shift VX left once
    fn op_8XYE(&mut self, x: usize, y: usize) -> u16 {
        self.v[0x0F] = (self.v[x] & 0x80) >> 7;
        self.v[x] <<= 1;
        self.next_pc()
    }

    // Skip if VX != VY
    fn op_9XY0(&mut self, x: usize, y: usize) -> u16 {
        if self.v[x] != self.v[y] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Set I to NNN
    fn op_ANNN(&mut self, nnn: u16) -> u16 {
        self.i = nnn;
        self.next_pc()
    }

    // Jump to NNN + V0
    fn op_BNNN(&mut self, nnn: u16) -> u16 {
        self.pc = nnn + self.v[0x00] as u16;
        self.pc
    }

    // Set VX to NN anded with a random number (0 to 255)
    fn op_CXNN(&mut self, x: usize, nn: u8) -> u16 {
        let mut rng = rand::thread_rng();
        self.v[x] = nn & rng.gen::<u8>();
        self.next_pc()
    }

    //TODO
    // Draw a sprite
    fn op_DXYN(&mut self, x: usize, y: usize, n: u8) -> u16 {
        self.gfx_changed = true;
        self.next_pc()
    }

    // Skip if key VX is pressed
    fn op_EX9E(&mut self, x: usize) -> u16 {
        if self.key[self.v[x] as usize] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Skip if key stored in VX isn't pressed
    fn op_EXA1(&mut self, x: usize) -> u16 {
        if !self.key[self.v[x] as usize] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Set VX to delay timer
    fn op_FX07(&mut self, x: usize) -> u16 {
        self.v[x] = self.delay_timer;
        self.next_pc()
    }

    //TODO
    // Store next key press in VX and block until key is pressed
    fn op_FX0A(&mut self, x: usize) -> u16 {
        self.next_pc()
    }

    // Set delay timer to VX
    fn op_FX15(&mut self, x: usize) -> u16 {
        self.delay_timer = self.v[x];
        self.next_pc()
    }

    // Set sound timer to VX
    fn op_FX18(&mut self, x: usize) -> u16 {
        self.sound_timer = self.v[x];
        self.next_pc()
    }

    // Add VX to I and set VF as carry bit
    fn op_FX1E(&mut self, x: usize) -> u16 {
        let sum = self.v[x] as u32 + self.i as u32;
        self.i = sum as u16;
        self.v[0x0F] = (sum >> 16) as u8;
        self.next_pc()
    }

    // Set I to location of sprite VX
    fn op_FX29(&mut self, x: usize) -> u16 {
        self.i = (self.v[x] as u16) * 5;
        self.next_pc()
    }

    // convert VX to decimal and store the three digits in ram at I, I+1, I+2
    fn op_FX33(&mut self, x: usize) -> u16 {
        let vx = self.v[x];
        self.ram[self.i as usize] = (vx / 100) % 10; // hundreds
        self.ram[(self.i + 1) as usize] = (vx / 10) % 10; // tens
        self.ram[(self.i + 2) as usize] = vx % 10; // ones
        self.next_pc()
    }

    // Store V0 to VX in ram starting at I
    fn op_FX55(&mut self, x: usize) -> u16 {
        let max_reg = match x {
            0x00...0x0F => x,
            _ => NUM_REGISTERS - 1, //TODO print warning about out of bounds
        };
        for reg in 0..max_reg + 1 {
            //TODO check for out of bounds on ram
            self.ram[(self.i as usize) + reg] = self.v[reg];
        }
        self.next_pc()
    }

    // Load V0 to VX with ram values starting at I
    fn op_FX65(&mut self, x: usize) -> u16 {
        let max_reg = match x {
            0x00...0x0F => x,
            _ => NUM_REGISTERS - 1, //TODO print warning
        };
        for reg in 0..max_reg + 1 {
            //TODO check for out of bounds on ram
            self.v[reg] = self.ram[(self.i as usize) + reg];
        }
        self.next_pc()
    }

    pub fn debug(&self) {
        println!("=== debug start ===");
        println!("  pc = 0x{:X}", self.pc);
        for i in 0..NUM_REGISTERS {
            println!("v[{:X}] = 0x{:X}", i, self.v[i]);
        }
    }
}

fn main() {
    let mut cpu = Simulator::new();
    /*
    cpu.debug();
    //cpu.step(0x00E0);
    cpu.step(0x601A);
    //cpu.step(0x6105);
    cpu.step(0xF033);
    cpu.debug();
    */

    let mut file = File::open("roms/TETRIS").unwrap();
    let mut buf = [0u8; RAM_SIZE];
    file.read(&mut buf).unwrap();
    cpu.load_program(&mut buf);
    loop {
        let opcode = cpu.get_opcode();
        cpu.step(opcode);
        cpu.debug();
    }
}
