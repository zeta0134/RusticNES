// Note: For basic testing purposes, this is scanline-accurate. This should
// later be rewritten with cycle-accurate logic once we're past proof of concept
// and prototype stages.

use mmc::mapper::*;

pub struct PpuState {
    // PPU Memory (incl. cart CHR ROM for now)
    pub internal_vram: Vec<u8>,
    pub oam: Vec<u8>,
    pub palette: Vec<u8>,

    // Memory Mapped Registers
    // PPU Registers
    pub latch: u8,

    pub read_buffer: u8,

    pub control: u8,
    pub mask: u8,
    pub status: u8,
    pub oam_addr: u8,

    // Scrolling, which is implemented with a flip/flop register
    pub scroll_x: u8,
    pub scroll_y: u8,

    // PPU Address, similar to scrolling, has a high / low component
    pub current_addr: u16,

    pub oam_dma_high: u8,

    // Internal
    pub current_frame: u32,
    pub current_scanline: u16,
    pub scanline_cycles: u64,
    pub last_cycle: u64,
    // 

    // Framebuffer
    pub screen: Vec<u8>,
    pub sprite_color: Vec<u8>,
    pub sprite_index: Vec<u8>,
    pub sprite_bg_priority: Vec<bool>,
    pub sprite_zero: Vec<bool>,

    pub write_toggle: bool,

    // Internal State
    pub current_vram_address: u16,
    pub temporary_vram_address: u16,
    pub fine_x: u8,
    pub fine_y: u8,
}

impl PpuState {
    pub fn new() -> PpuState {
        return PpuState {
           internal_vram: vec!(0u8; 0x1000),  // 4k for four-screen mirroring, most games only use upper 2k
           oam: vec!(0u8; 0x100),
           palette: vec!(0u8; 0x20),
           current_frame: 0,
           current_scanline: 0,
           scanline_cycles: 0,
           last_cycle: 0,
           screen: vec!(0u8; 256 * 240),
           sprite_color: vec!(0u8; 256),
           sprite_index: vec!(0u8; 256),
           sprite_bg_priority: vec!(false; 256),
           sprite_zero: vec!(false; 256),

           control: 0,
           mask: 0,
           status: 0,
           oam_addr: 0,
           scroll_x: 0,
           scroll_y: 0,
           current_addr: 0,
           oam_dma_high: 0,
           latch: 0,
           read_buffer: 0,

           write_toggle: false,

           // Internal State
           current_vram_address: 0,
           temporary_vram_address: 0,
           fine_x: 0,
           fine_y: 0,
       };
    }

    pub fn read_byte(&mut self, mapper: &mut Mapper, address: u16) -> u8 {
        let masked_address = address & 0x3FFF;
        match masked_address {
            0x3F00 ... 0x3FFF => {
                // Weird read buffer behavior
                self.read_buffer = self.read_byte(mapper, (masked_address & 0x0FFF) + 0x2000);
                return self._read_byte(mapper, address);
            },
            _ => {
                let result = self.read_buffer;
                self.read_buffer = self._read_byte(mapper, address);
                return result;
            }
        }
    }

    pub fn _read_byte(&mut self, mapper: &mut Mapper, address: u16) -> u8 {
        let masked_address = address & 0x3FFF;
        match masked_address {
            0x0000 ... 0x1FFF => return mapper.read_byte(masked_address),
            // Nametable 0 (top-left)
            0x2000 ... 0x2FFF => return self.internal_vram[nametable_address(masked_address, mapper.mirroring()) as usize],
            0x3000 ... 0x3EFF => return self.read_byte(mapper, masked_address - 0x1000),
            0x3F00 ... 0x3FFF => {
                // Weird read buffer behavior
                //self.read_buffer = self.read_byte((masked_address & 0x0FFF) + 0x2000);

                let mut palette_address = masked_address & 0x1F;
                // Weird background masking
                if palette_address & 0x13 == 0x10 {
                    palette_address = palette_address - 0x10;
                }
                return self.palette[palette_address as usize];
            },
            _ => return 0
        }
    }
    pub fn write_byte(&mut self, mapper: &mut Mapper, address: u16, data: u8) {
        let masked_address = address & 0x3FFF;
        match masked_address {
            0x0000 ... 0x1FFF => mapper.write_byte(address, data),
            // Nametable 0
            0x2000 ... 0x2FFF => self.internal_vram[nametable_address(masked_address, mapper.mirroring()) as usize] = data,
            0x3000 ... 0x3EFF => self.write_byte(mapper, masked_address - 0x1000, data),
            0x3F00 ... 0x3FFF => {
                // palette data is 6-bits, so mask off the upper two:
                let palette_entry = data & 0b0011_1111;
                let mut palette_address = masked_address & 0x1F;
                // Weird background masking
                if palette_address & 0x13 == 0x10 {
                    palette_address = palette_address - 0x10;
                }
                self.palette[palette_address as usize] = palette_entry;
            },
            _ => () // Do nothing!
        }
    }

    fn render_sprites(&mut self, mapper: &mut Mapper, scanline: u8) {
        // Init buffers
        self.sprite_color = vec!(0u8; 256);
        self.sprite_index = vec!(0u8; 256);
        self.sprite_bg_priority = vec!(false; 256);
        self.sprite_zero = vec!(false; 256);

        let mut secondary_oam = [0xFFu8; 32];
        let mut secondary_index = 0;
        let mut sprite_size = 8;
        if (self.control & 0x20) != 0 {
            sprite_size = 16;
        }
        let mut sprite_zero_on_scanline = false;

        // Gather first 8 visible sprites (and pay attention if there are more)
        for i in 0 .. 64 {
            let y = self.oam[i * 4 + 0];
            if scanline >= y && scanline < y + sprite_size {
                if secondary_index < 8 {
                    for j in 0 .. 4 {
                        secondary_oam[secondary_index * 4 + j] = self.oam[i * 4 + j];
                    }
                    secondary_index += 1;
                    if i == 0 {
                        sprite_zero_on_scanline = true;
                    }
                } else {
                    self.status = self.status | 0x20; // bit 5 = sprite overflow this frame
                }
            }
        }

        // secondary_oam now has up to 8 sprites, all of which have a Y coordinate
        // which is on this scanline. Proceed to render!

        // Note: Iterating over the list in reverse order cheats a bit, by having higher priority
        // sprites overwrite the work done to draw lower priority sprites.
        for i in (0 .. secondary_index).rev() {
            let mut pattern_address = 0x0000;
            let sprite_y = secondary_oam[i * 4 + 0];
            let mut tile_index = secondary_oam[i * 4 + 1];
            let flags = secondary_oam[i * 4 + 2];
            let sprite_x = secondary_oam[i * 4 + 3];

            let priority = flags & 0x20 != 0;
            let mut tile_y = scanline - sprite_y;
            if sprite_size == 16 {
                if tile_index & 0x01 != 0 {
                    pattern_address = 0x1000;
                }
                tile_index = tile_index & 0xFE;
                if flags & 0x80 != 0 {
                    tile_y = 15 - tile_y;
                }
                if tile_y >= 8 {
                    tile_y -= 8;
                    tile_index += 1;
                }
            } else {
                if flags & 0x80 != 0 {
                    tile_y = 7 - tile_y;
                }
                if (self.control & 0x08) != 0 {
                    pattern_address = 0x1000;
                }
            }

            let palette_index = flags & 0x03;

            for x in 0 .. 8 {
                let scanline_x = (sprite_x as u16) + x;
                let mut tile_x = x;
                if flags & 0x40 != 0 {
                    tile_x = 7 - tile_x;
                }

                if scanline_x < 256 {
                    let chr_index = decode_chr_pixel(mapper, pattern_address, tile_index, tile_x as u8, tile_y);
                    if chr_index > 0 {
                        let palette_color = self._read_byte(mapper, ((palette_index << 2) + chr_index) as u16 + 0x3F10);
                        self.sprite_index[scanline_x as usize] = chr_index;
                        self.sprite_color[scanline_x as usize] = palette_color;
                        self.sprite_bg_priority[scanline_x as usize] = priority;
                        self.sprite_zero[scanline_x as usize] = (i == 0) && sprite_zero_on_scanline;
                    }
                }
            }
        }
    }

    fn render_background(&mut self, mapper: &mut Mapper, scanline: u16) {
        let mut pattern_address = 0x0000;
        if (self.control & 0x10) != 0 {
            pattern_address = 0x1000;
        }
        let mut y = self.scroll_y as u16 + scanline;
        if self.control & 0x2 != 0 {
            y += 240;
        }
        // wrap around if we go off the bottom of the map
        y = y % (240 * 2);
        for sx in 0 .. 256 {
            let mut x = sx + self.scroll_x as u16;
            if self.control & 0x1 != 0 {
                x += 256;
            }
            // wrap around if we go off the right of the map
            x = x & 0x1FF;
            let tx = (x >> 3) as u8;
            let ty = (y >> 3) as u8;
            let tile = self.get_bg_tile(mapper, tx, ty);
            let bg_index = decode_chr_pixel(mapper, pattern_address, tile, (x & 0x7) as u8, (y & 0x7) as u8);
            let mut palette_index = self.get_bg_palette(mapper, tx, ty);
            if bg_index == 0 {
                palette_index = 0; // Ignore palette index for color 0
            }
            let palette_color = self._read_byte(mapper, ((palette_index << 2) + bg_index) as u16 + 0x3F00);
            if self.mask & 0x08 != 0 {
                self.screen[(scanline * 256 + sx) as usize] = palette_color;
            } else {
                // Not positive on this, is this documented somewhere?
                self.screen[(scanline * 256 + sx) as usize] = self._read_byte(mapper, 0x3F00);
            }

            // Here, decide if a sprite pixel should overwrite a background pixel
            if self.mask & 0x10 != 0 {
                if self.sprite_index[sx as usize] != 0 {
                    if bg_index == 0 || !self.sprite_bg_priority[sx as usize] {
                        self.screen[(scanline * 256 + sx) as usize] = self.sprite_color[sx as usize];
                    }
                    if self.sprite_zero[sx as usize] {
                        self.status = self.status | 0x40; // bit 6 = sprite zero hit
                        //self.screen[(scanline * 256 + sx) as usize] = 0x25;
                    }
                }
            }
        }
    }

    // Return value: NMI interrupt happened this cycle
    pub fn process_scanline(&mut self, mapper: &mut Mapper) {
        let scanline = self.current_scanline;
        match scanline {
            0 ... 239 => {
                // Visible scanline here
                self.render_sprites(mapper, (scanline - 1) as u8);
                self.render_background(mapper, scanline);
            },
            // 240 does nothing
            241 => {
                // VBlank! Set NMI flag here
                self.status = (self.status & 0x7F) + 0x80; // Set VBlank bit
            },
            // 242 - 260 do nothing
            261 => {
                // Set vertical scrolling registers, in preparation for new frame
                // (Emulator: Draw actual frame here)
                // Clear sprite overflow and sprite zero hit too
                self.status = self.status & 0x1F; // Clear VBlank bit
            },
            _ => ()
        }
    }

    pub fn clock(&mut self, mapper: &mut Mapper) {
        let cycles_per_scanline = 341;
        self.scanline_cycles = self.scanline_cycles + 1;
        while self.scanline_cycles > cycles_per_scanline {
            self.process_scanline(mapper);
            self.current_scanline = self.current_scanline.wrapping_add(1);
            if self.current_scanline > 261 {
                self.current_scanline = 0;
                self.current_frame = self.current_frame + 1;
            }
            self.scanline_cycles = self.scanline_cycles  - cycles_per_scanline;
        }
    }

    pub fn get_bg_tile(&mut self, mapper: &mut Mapper, tx: u8, ty: u8) -> u8 {
        let mut address: u16 = 0x2000;
        if tx > 31 {
            address = address + 0x0400;
        }
        if ty > 29 {
            address = address + 0x0800;
        }
        address = address + ((ty % 30) as u16) * 32 + ((tx & 0x1F) as u16);
        return self._read_byte(mapper, address);
    }

    pub fn get_bg_palette(&mut self, mapper: &mut Mapper, tx: u8, ty: u8) -> u8 {
        let mut address: u16 = 0x23C0;
        if tx > 31 {
            address = address + 0x0400;
        }
        if ty > 29 {
            address = address + 0x0800;
        }
        address += ((tx & 0x1F) >> 2) as u16;
        address += (((ty % 30) >> 2) as u16)* 0x8;
        let attr_byte = self._read_byte(mapper, address);
        let shift = (((tx & 0x2) >> 1) + ((ty % 30) & 0x2)) << 1;
        let mask = 0x3 << shift;
        return (attr_byte & mask) >> shift;
    }
}

pub fn nametable_address(read_address: u16, mirroring: Mirroring) -> u16 {
    // Mirroring documented here, the ABCD references to the charts in this article:
    // https://wiki.nesdev.com/w/index.php/Mirroring
    let nt_address = read_address & 0x3FF;
    let nt_offset = (0x000, 0x400, 0x800, 0xC00);
    match read_address {
        // Nametable 0 (top-left)
        0x2000 ... 0x23FF => {
            return match mirroring {
                Mirroring::Horizontal       => nt_address + nt_offset.0, // A
                Mirroring::Vertical         => nt_address + nt_offset.0, // A
                Mirroring::OneScreenLower   => nt_address + nt_offset.0, // A
                Mirroring::OneScreenUpper   => nt_address + nt_offset.1, // B
                Mirroring::FourScreen       => nt_address + nt_offset.0, // A
            }
        },
        // Nametable 1 (top-right)
        0x2400 ... 0x27FF => {
            return match mirroring {
                Mirroring::Horizontal       => nt_address + nt_offset.0, // A
                Mirroring::Vertical         => nt_address + nt_offset.1, // B
                Mirroring::OneScreenLower   => nt_address + nt_offset.0, // A
                Mirroring::OneScreenUpper   => nt_address + nt_offset.1, // B
                Mirroring::FourScreen       => nt_address + nt_offset.1, // B
            }
        },
        // Nametable 2 (bottom-left)
        0x2800 ... 0x2BFF => {
            return match mirroring {
                Mirroring::Horizontal       => nt_address + nt_offset.1, // B
                Mirroring::Vertical         => nt_address + nt_offset.0, // A
                Mirroring::OneScreenLower   => nt_address + nt_offset.0, // A
                Mirroring::OneScreenUpper   => nt_address + nt_offset.1, // B
                Mirroring::FourScreen       => nt_address + nt_offset.2, // C
            }
        },
        // Nametable 3 (bottom-right)
        0x2C00 ... 0x2FFF => {
            return match mirroring {
                Mirroring::Horizontal       => nt_address + nt_offset.1, // B
                Mirroring::Vertical         => nt_address + nt_offset.1, // B
                Mirroring::OneScreenLower   => nt_address + nt_offset.0, // A
                Mirroring::OneScreenUpper   => nt_address + nt_offset.1, // B
                Mirroring::FourScreen       => nt_address + nt_offset.3, // D
            }
        },
        _ => return 0, // wat
    }
}

// Given a pattern and tile / pixel coordinates, decodes the palette index and returns it
// (Palette index will be between 0 .. 3)
pub fn decode_chr_pixel(mapper: &Mapper, pattern_address: u16, tile: u8, pixel_x: u8, pixel_y: u8) -> u8 {
    let low_addr = (tile as u16) * 16 + (pixel_y as u16);
    let high_addr = low_addr + 8;
    let low_bit = mapper.read_byte(pattern_address + low_addr) >> (7 - pixel_x) & 0x1;
    let high_bit = mapper.read_byte(pattern_address + high_addr) >> (7 - pixel_x) & 0x1;
    return (high_bit << 1) + low_bit;
}
