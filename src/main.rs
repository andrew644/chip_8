const RAM_SIZE: usize = 4096;
const NUM_REGISTERS: usize = 16;
const NUM_PIXELS: usize = 64 * 32;
const STACK_SIZE: usize = 16;
const NUM_KEYS: usize = 16;

pub struct Cpu {
    ram: [u8; RAM_SIZE],

    v: [u8; NUM_REGISTERS], // registers
    i: u16,                 // index register
    pc: u16,

    stack: [u16; STACK_SIZE],
    sp: u16,

    gfx: [u8; NUM_PIXELS],

    delay_timer: u8,
    sound_timer: u8,

    key: [bool; NUM_KEYS],
}

impl Cpu {
    pub fn new() -> Self {
        let mut ram = [0u8; RAM_SIZE];
        //TODO load font set

        Cpu {
            ram: ram,
            v: [0; NUM_REGISTERS],
            i: 0,
            pc: 0x200,
            stack: [0; STACK_SIZE],
            sp: 0,
            gfx: [0; NUM_PIXELS],
            delay_timer: 0,
            sound_timer: 0,
            key: [false; NUM_KEYS],
        }
    }

    pub fn load_program(&mut self, program: &[u8]) {}

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
            _ => self.next_pc(), //TODO message that we couldn't find opcode
        }
    }

    // Clear the display
    fn op_00E0(&mut self) -> u16 {
        for i in 0..NUM_PIXELS {
            self.gfx[i] = 0
        }
        //TODO set changed flag for gfx
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

    pub fn debug(&self) {
        println!("=== debug start ===");
        println!("pc:{}", self.pc);
    }
}

fn main() {
    let mut cpu = Cpu::new();
    cpu.debug();
    //cpu.step(0x00E0);
    cpu.step(0x3FEA);
    cpu.debug();
}
