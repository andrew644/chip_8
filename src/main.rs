const RAM_SIZE: usize = 4096;
const NUM_REGISTERS: usize = 16;
const NUM_PIXELS: usize = 64 * 32;
const STACK_SIZE: usize = 16;
const NUM_KEYS: usize = 16;

pub struct Cpu {
    ram: [u8; RAM_SIZE],

    reg: [u8; NUM_REGISTERS],
    index_reg: u16,
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
            reg: [0; NUM_REGISTERS],
            index_reg: 0,
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

    fn next_pc(&mut self) {
        self.pc += 2
    }

    pub fn step(&mut self, opcode: u16) {
        let nibbles = (
            (opcode & 0xF000) >> 12 as u8,
            (opcode & 0x0F00) >> 8 as u8,
            (opcode & 0x00F0) >> 4 as u8,
            (opcode & 0x000F) as u8,
        );

        match nibbles {
            (0x00, 0x00, 0x0E, 0x00) => self.op_00E0(),
            (0x00, 0x00, 0x0E, 0x0E) => self.op_00EE(),
            _ => self.next_pc(),
        }
    }

    // Clear the display
    fn op_00E0(&mut self) {
        for i in 0..NUM_PIXELS {
            self.gfx[i] = 0
        }
        //TODO set changed flag for gfx
        self.next_pc();
    }

    // Return from subroutine
    fn op_00EE(&mut self) {
        self.sp -= 1;
        self.pc = self.stack[self.sp as usize];
        self.next_pc();
    }

    pub fn debug(&self) {
        println!("=== debug start ===");
        println!("pc:{}", self.pc);
    }
}

fn main() {
    let mut cpu = Cpu::new();
    cpu.debug();
    cpu.step(0x00E0);
    cpu.debug();
}
