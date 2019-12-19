extern crate ggez;
extern crate rand;

use ggez::conf;
use ggez::event;
use ggez::graphics;
use ggez::nalgebra as na;
use ggez::{Context, GameResult};

use std::env;
use std::fs::File;
use std::io::Read;

use rand::Rng;

const RAM_SIZE: usize = 0x1000; // 4096
const NUM_REGISTERS: usize = 16;
const SCREEN_HEIGHT: usize = 32;
const SCREEN_WIDTH: usize = 64;
const STACK_SIZE: usize = 16;
const NUM_KEYS: usize = 16;
const START_PC: usize = 0x200; // 512

const PIXEL_SIZE: f32 = 10.0;

// Holds caracters 0 to F
// Each character is 5 bytes
const FONT_SET: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80, 0xF0, 0xF0,
    0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0, 0x10, 0xF0, 0xF0, 0x80,
    0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90, 0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0,
    0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0, 0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80,
    0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0, 0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
];

struct MainState {
    sim: Simulator,
}

impl MainState {
    #[allow(dead_code)]
    fn new(_ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            sim: Simulator::new(),
        };
        Ok(s)
    }
    fn new_with_sim(_ctx: &mut Context, simulator: Simulator) -> GameResult<MainState> {
        let s = MainState { sim: simulator };
        Ok(s)
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, _ctx: &mut Context) -> GameResult {
        let opcode = self.sim.get_opcode();
        self.sim.step(opcode);
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        graphics::clear(ctx, [0.1, 0.2, 0.3, 1.0].into());

        let circle = graphics::Mesh::new_rectangle(
            ctx,
            graphics::DrawMode::fill(),
            graphics::Rect::new(0.0, 0.0, PIXEL_SIZE, PIXEL_SIZE),
            graphics::WHITE,
        )?;
        for (y, row) in self.sim.screen.iter().enumerate() {
            for (x, pixel) in row.iter().enumerate() {
                if *pixel != 0x00 {
                    graphics::draw(
                        ctx,
                        &circle,
                        (na::Point2::new(
                            0.0 + (x as f32 * PIXEL_SIZE),
                            0.0 + (y as f32 * PIXEL_SIZE),
                        ),),
                    )?;
                }
            }
        }

        graphics::present(ctx)?;
        Ok(())
    }
}

pub struct Simulator {
    ram: [u8; RAM_SIZE],

    v: [u8; NUM_REGISTERS], // registers
    i: u16,                 // index register
    pc: u16,                // program counter

    stack: [u16; STACK_SIZE],
    sp: u16, // stack pointer

    screen: [[u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
    gfx_changed: bool,

    delay_timer: u8,
    sound_timer: u8,

    key: [bool; NUM_KEYS], // keypad
    await_key: Option<usize>,
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
            screen: [[0; SCREEN_WIDTH]; SCREEN_HEIGHT],
            gfx_changed: false,
            delay_timer: 0,
            sound_timer: 0,
            key: [false; NUM_KEYS],
            await_key: None,
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
            (0x00, 0x00, 0x0E, 0x00) => self.op_00e0(),
            (0x00, 0x00, 0x0E, 0x0E) => self.op_00ee(),
            (0x01, _, _, _) => self.op_1nnn(nnn),
            (0x02, _, _, _) => self.op_2nnn(nnn),
            (0x03, x, _, _) => self.op_3xnn(x as usize, nn),
            (0x04, x, _, _) => self.op_4xnn(x as usize, nn),
            (0x05, x, y, 0x00) => self.op_5xy0(x as usize, y as usize),
            (0x06, x, _, _) => self.op_6xnn(x as usize, nn),
            (0x07, x, _, _) => self.op_7xnn(x as usize, nn),
            (0x08, x, y, 0x00) => self.op_8xy0(x as usize, y as usize),
            (0x08, x, y, 0x01) => self.op_8xy1(x as usize, y as usize),
            (0x08, x, y, 0x02) => self.op_8xy2(x as usize, y as usize),
            (0x08, x, y, 0x03) => self.op_8xy3(x as usize, y as usize),
            (0x08, x, y, 0x04) => self.op_8xy4(x as usize, y as usize),
            (0x08, x, y, 0x05) => self.op_8xy5(x as usize, y as usize),
            (0x08, x, _, 0x06) => self.op_8xy6(x as usize),
            (0x08, x, y, 0x07) => self.op_8xy7(x as usize, y as usize),
            (0x08, x, _, 0x0E) => self.op_8xye(x as usize),
            (0x09, x, y, 0x00) => self.op_9xy0(x as usize, y as usize),
            (0x0A, _, _, _) => self.op_annn(nnn),
            (0x0B, _, _, _) => self.op_bnnn(nnn),
            (0x0C, x, _, _) => self.op_cxnn(x as usize, nn),
            (0x0D, x, y, n) => self.op_dxyn(x as usize, y as usize, n as u8),
            (0x0E, x, 0x09, 0x0E) => self.op_ex9e(x as usize),
            (0x0E, x, 0x0A, 0x01) => self.op_exa1(x as usize),
            (0x0F, x, 0x00, 0x07) => self.op_fx07(x as usize),
            (0x0F, x, 0x00, 0x0A) => self.op_fx0a(x as usize),
            (0x0F, x, 0x01, 0x05) => self.op_fx15(x as usize),
            (0x0F, x, 0x01, 0x08) => self.op_fx18(x as usize),
            (0x0F, x, 0x01, 0x0E) => self.op_fx1e(x as usize),
            (0x0F, x, 0x02, 0x09) => self.op_fx29(x as usize),
            (0x0F, x, 0x03, 0x03) => self.op_fx33(x as usize),
            (0x0F, x, 0x05, 0x05) => self.op_fx55(x as usize),
            (0x0F, x, 0x06, 0x05) => self.op_fx65(x as usize),
            _ => self.next_pc(), //TODO message that we couldn't find opcode
        }
    }

    // Clear the display
    fn op_00e0(&mut self) -> u16 {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                self.screen[y][x] = 0;
            }
        }
        self.gfx_changed = true;
        self.next_pc()
    }

    // Return from subroutine
    fn op_00ee(&mut self) -> u16 {
        self.sp -= 1;
        self.pc = self.stack[self.sp as usize];
        self.next_pc()
    }

    // Jump to NNN
    fn op_1nnn(&mut self, nnn: u16) -> u16 {
        self.pc = nnn;
        self.pc
    }

    // Call subroutine NNN
    fn op_2nnn(&mut self, nnn: u16) -> u16 {
        self.sp += 1;
        self.stack[self.sp as usize] = self.pc;
        //TODO check for stack overflow?
        self.pc = nnn;
        self.pc
    }

    // Skip if VX == NN
    fn op_3xnn(&mut self, x: usize, nn: u8) -> u16 {
        if self.v[x] == nn {
            self.next_pc();
        }
        self.next_pc()
    }

    // Skip if VX != NN
    fn op_4xnn(&mut self, x: usize, nn: u8) -> u16 {
        if self.v[x] != nn {
            self.next_pc();
        }
        self.next_pc()
    }

    // Skip if VX == VY
    fn op_5xy0(&mut self, x: usize, y: usize) -> u16 {
        if self.v[x] == self.v[y] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Set VX to NN
    fn op_6xnn(&mut self, x: usize, nn: u8) -> u16 {
        self.v[x] = nn;
        self.next_pc()
    }

    // Add NN to VX
    fn op_7xnn(&mut self, x: usize, nn: u8) -> u16 {
        //TODO is this needed?
        // Do the addition as u16 then truncate to u8 so we don't roll over
        self.v[x] = ((self.v[x] as u16) + (nn as u16)) as u8;
        self.next_pc()
    }

    // Set VX to VY
    fn op_8xy0(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] = self.v[y];
        self.next_pc()
    }

    // Set VX to VX or VY
    fn op_8xy1(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] |= self.v[y];
        self.next_pc()
    }

    // Set VX to VX and VY
    fn op_8xy2(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] &= self.v[y];
        self.next_pc()
    }

    // Set VX to VX xor VY
    fn op_8xy3(&mut self, x: usize, y: usize) -> u16 {
        self.v[x] ^= self.v[y];
        self.next_pc()
    }

    // Add VY to VX and use VF as carry bit
    // 1 for carry, 0 otherwise
    fn op_8xy4(&mut self, x: usize, y: usize) -> u16 {
        let sum = self.v[x] as u16 + self.v[y] as u16;
        self.v[x] = sum as u8;
        self.v[0x0F] = (sum >> 8) as u8;
        self.next_pc()
    }

    // Subtract VY from VX and use VF as borrow bit
    // 0 for borrow, 1 otherwise
    fn op_8xy5(&mut self, x: usize, y: usize) -> u16 {
        let borrow = self.v[y] > self.v[x];
        self.v[x] = self.v[x].wrapping_sub(self.v[y]);
        self.v[0x0F] = !borrow as u8;
        self.next_pc()
    }

    // Put least significant bit from VX in VF then shift VX right once
    fn op_8xy6(&mut self, x: usize) -> u16 {
        self.v[0x0F] = self.v[x] & 0x01;
        self.v[x] >>= 1;
        self.next_pc()
    }

    // Set VX to VY - VX and use VF as borrow bit
    // 0 for borrow, 1 otherwise
    fn op_8xy7(&mut self, x: usize, y: usize) -> u16 {
        let borrow = self.v[x] > self.v[y];
        self.v[x] = self.v[y].wrapping_sub(self.v[x]);
        self.v[0x0F] = !borrow as u8;
        self.next_pc()
    }

    // Put most significant bit from VX in VF then shift VX left once
    fn op_8xye(&mut self, x: usize) -> u16 {
        self.v[0x0F] = (self.v[x] & 0x80) >> 7;
        self.v[x] <<= 1;
        self.next_pc()
    }

    // Skip if VX != VY
    fn op_9xy0(&mut self, x: usize, y: usize) -> u16 {
        if self.v[x] != self.v[y] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Set I to NNN
    fn op_annn(&mut self, nnn: u16) -> u16 {
        self.i = nnn;
        self.next_pc()
    }

    // Jump to NNN + V0
    fn op_bnnn(&mut self, nnn: u16) -> u16 {
        self.pc = nnn + self.v[0x00] as u16;
        self.pc
    }

    // Set VX to NN anded with a random number (0 to 255)
    fn op_cxnn(&mut self, x: usize, nn: u8) -> u16 {
        let mut rng = rand::thread_rng();
        self.v[x] = nn & rng.gen::<u8>();
        self.next_pc()
    }

    // Draw a sprite at (VX, VY)
    // Start at I in ram and draw N lines
    // Each line is 8 pixels (1 byte from memeroy)
    // Set VF to 1 if we overwrite a pixel that was already on
    fn op_dxyn(&mut self, x: usize, y: usize, n: u8) -> u16 {
        self.v[0x0F] = 0;
        for line in 0..n as usize {
            let y = (self.v[y] as usize + line) % SCREEN_HEIGHT;
            for pixel in 0..8 {
                let x = (self.v[x] as usize + pixel) % SCREEN_WIDTH;
                let new_pixel = (self.ram[self.i as usize + line] >> (7 - pixel)) & 0x01;
                self.v[0x0F] |= new_pixel & self.screen[y][x];
                self.screen[y][x] ^= new_pixel;
            }
        }
        self.gfx_changed = true;
        self.next_pc()
    }

    // Skip if key VX is pressed
    fn op_ex9e(&mut self, x: usize) -> u16 {
        if self.key[self.v[x] as usize] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Skip if key stored in VX isn't pressed
    fn op_exa1(&mut self, x: usize) -> u16 {
        if !self.key[self.v[x] as usize] {
            self.next_pc();
        }
        self.next_pc()
    }

    // Set VX to delay timer
    fn op_fx07(&mut self, x: usize) -> u16 {
        self.v[x] = self.delay_timer;
        self.next_pc()
    }

    // Store next key press in VX and block until key is pressed
    fn op_fx0a(&mut self, x: usize) -> u16 {
        self.await_key = Some(x);
        self.next_pc()
    }

    // Set delay timer to VX
    fn op_fx15(&mut self, x: usize) -> u16 {
        self.delay_timer = self.v[x];
        self.next_pc()
    }

    // Set sound timer to VX
    fn op_fx18(&mut self, x: usize) -> u16 {
        self.sound_timer = self.v[x];
        self.next_pc()
    }

    // Add VX to I and set VF as carry bit
    fn op_fx1e(&mut self, x: usize) -> u16 {
        let sum = self.v[x] as u32 + self.i as u32;
        self.i = sum as u16;
        self.v[0x0F] = (sum >> 16) as u8;
        self.next_pc()
    }

    // Set I to location of sprite VX
    fn op_fx29(&mut self, x: usize) -> u16 {
        self.i = (self.v[x] as u16) * 5;
        self.next_pc()
    }

    // convert VX to decimal and store the three digits in ram at I, I+1, I+2
    fn op_fx33(&mut self, x: usize) -> u16 {
        let vx = self.v[x];
        self.ram[self.i as usize] = (vx / 100) % 10; // hundreds
        self.ram[(self.i + 1) as usize] = (vx / 10) % 10; // tens
        self.ram[(self.i + 2) as usize] = vx % 10; // ones
        self.next_pc()
    }

    // Store V0 to VX in ram starting at I
    fn op_fx55(&mut self, x: usize) -> u16 {
        for reg in 0..x + 1 {
            //TODO check for out of bounds on ram
            self.ram[(self.i as usize) + reg] = self.v[reg];
        }
        self.next_pc()
    }

    // Load V0 to VX with ram values starting at I
    fn op_fx65(&mut self, x: usize) -> u16 {
        for reg in 0..x + 1 {
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

fn main() -> GameResult {
    let args: Vec<String> = env::args().collect();

    let mut cpu = Simulator::new();

    let mut file = File::open(&args[1]).unwrap();
    let mut buf = [0u8; RAM_SIZE];
    file.read(&mut buf).unwrap();
    cpu.load_program(&mut buf);

    let cb = ggez::ContextBuilder::new("chip8", "ggez")
        .window_setup(conf::WindowSetup::default().title("Chip8"))
        .window_mode(conf::WindowMode::default().dimensions(
            SCREEN_WIDTH as f32 * PIXEL_SIZE,
            SCREEN_HEIGHT as f32 * PIXEL_SIZE,
        ));
    let (ctx, event_loop) = &mut cb.build()?;
    let state = &mut MainState::new_with_sim(ctx, cpu)?;
    event::run(ctx, event_loop, state)
}
