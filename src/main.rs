mod cpu;

use std::error::Error;
use std::io::Read;
use std::fs::File;

struct NesHeader {
  prg_rom_size: u32,
  chr_rom_size: u32,
  mapper_number: u8,
  prg_ram_size: u32,

  // Flags 6
  horizontal_mirroring: bool,
  vertical_mirroring: bool,
  has_sram: bool,
  trainer: bool,
  four_screen_mirroring: bool,
}

impl Default for NesHeader {
    fn default() -> NesHeader {
        NesHeader {
            prg_rom_size: 0,
            chr_rom_size: 0,
            mapper_number: 0,
            prg_ram_size: 0,

            horizontal_mirroring: false,
            vertical_mirroring: false,
            has_sram: false,
            trainer: false,
            four_screen_mirroring: false,
        }
    }
}

fn print_program_state(registers: &cpu::Registers, memory: &[u8]) {
    println!("A: {:X} X: {:X} Y: {:X} ", registers.a, registers.x, registers.y);
    println!("PC: {:X} S: {:X}", registers.pc, registers.s);
    println!("nv  dzic");
    println!("{:b}{:b}  {:b}{:b}{:b}{:b}",
        registers.flags.negative as u8,
        registers.flags.overflow as u8,
        registers.flags.decimal as u8,
        registers.flags.zero as u8,
        registers.flags.interrupts_disabled as u8,
        registers.flags.carry as u8,
    );
}

fn main() {
    println!("Hello, world!");
    println!("Attempting to read mario.nes header");

    let mut file = match File::open("mario.nes") {
        Err(why) => panic!("Couldn't open mario.nes: {}", why.description()),
        Ok(file) => file,
    };
    let mut cartridge = Vec::new();
    // Read the whole damn thing?
    match file.read_to_end(&mut cartridge) {
        Err(why) => panic!("Couldn't read data: {}", why.description()),
        Ok(bytes_read) => println!("Data read successfully: {}", bytes_read),
    };

    // See if that worked
    println!("Magic Header: {0} {1} {2} 0x{3:X}", cartridge[0] as char, cartridge[1] as char, cartridge[2] as char, cartridge[3]);

    // Okay, now create an NES struct and massage the data into it
    let mut nes_header: NesHeader = NesHeader {
        prg_rom_size: cartridge[4] as u32 * 16 * 1024,
        chr_rom_size: cartridge[5] as u32 * 8 * 1024,
        mapper_number: (cartridge[6] & 0xF0 >> 4) + cartridge[7] & 0xF0,
        prg_ram_size: cartridge[8] as u32 * 8 * 1024,
        ..Default::default()
    };

    println!("PRG ROM: {0}", nes_header.prg_rom_size);
    println!("CHR ROM: {0}", nes_header.chr_rom_size);
    println!("PRG RAM: {0}", nes_header.prg_ram_size);
    println!("Mapper: {0}", nes_header.mapper_number);

    if cartridge[6] & 0x08 != 0 {
        nes_header.four_screen_mirroring = true;
    } else {
        nes_header.horizontal_mirroring = cartridge[6] & 0x01 == 0;
        nes_header.vertical_mirroring   = cartridge[6] & 0x01 != 0;
    }
    nes_header.has_sram = cartridge[6] & 0x02 != 0;
    nes_header.trainer  = cartridge[6] & 0x04 != 0;

    let mut offset = 16;
    let mut trainer = &cartridge[16..16]; //default to empty
    if nes_header.trainer {
        trainer = &cartridge[offset..(offset + 512)];
        offset = offset + 512;
    }
    let prg_rom_size = (nes_header.prg_rom_size) as usize;
    let prg_rom = &cartridge[offset .. (offset + prg_rom_size)];
    offset = offset + prg_rom_size;

    let chr_rom_size = (nes_header.chr_rom_size) as usize;
    let chr_rom = &cartridge[offset .. (offset + chr_rom_size as usize)];
    offset = offset + chr_rom_size;

    let mut memory = [0u8;0x10000];
    let mut registers = cpu::Registers {
        a: 0,
        x: 0,
        y: 0,
        pc: 0,
        s: 0,
        flags: cpu::Flags {
            carry: false,
            zero: false,
            interrupts_disabled: false,
            decimal: false,
            overflow: false,
            negative: false,
        }
    };

    // Initialize main memory (this is only valid for very simple games)
    for i in 0 .. 32768 - 1 {
        memory[0x8000 + i] = prg_rom[i];
    }

    // Initialize CPU register state for power-up sequence
    registers.a = 0;
    registers.y = 0;
    registers.x = 0;
    registers.s = 0xFD;

    let pc_low = memory[0xFFFC];
    let pc_high = memory[0xFFFD];
    registers.pc = pc_low as u16 + ((pc_high as u16) << 8);

    // Initialized? Let's go!
    print_program_state(&registers, &memory);
    for i in 1 .. 10 {
        cpu::process_instruction(&mut registers, &mut memory);
        print_program_state(&registers, &memory);
    }


}
