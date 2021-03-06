use mmc::mapper::*;
use mmc::axrom::AxRom;
use mmc::bnrom::BnRom;
use mmc::cnrom::CnRom;
use mmc::fme7::Fme7;
use mmc::gxrom::GxRom;
use mmc::ines31::INes31;
use mmc::mmc1::Mmc1;
use mmc::mmc3::Mmc3;
use mmc::mmc5::Mmc5;
use mmc::nrom::Nrom;
use mmc::pxrom::PxRom;
use mmc::uxrom::UxRom;
use mmc::vrc6::Vrc6;

use ines::INesCartridge;

use std::io::Read;

fn mapper_from_ines(ines: INesCartridge) -> Result<Box<dyn Mapper>, String> {
    let mapper_number = ines.header.mapper_number();

    let mapper: Box<dyn Mapper> = match mapper_number {
        0 => Box::new(Nrom::from_ines(ines)?),
        1 => Box::new(Mmc1::from_ines(ines)?),
        2 => Box::new(UxRom::from_ines(ines)?),
        3 => Box::new(CnRom::from_ines(ines)?),
        4 => Box::new(Mmc3::from_ines(ines)?),
        5 => Box::new(Mmc5::from_ines(ines)?),
        7 => Box::new(AxRom::from_ines(ines)?),
        9 => Box::new(PxRom::from_ines(ines)?),
        24 => Box::new(Vrc6::from_ines(ines)?),
        26 => Box::new(Vrc6::from_ines(ines)?),
        31 => Box::new(INes31::from_ines(ines)?),
        34 => Box::new(BnRom::from_ines(ines)?),
        66 => Box::new(GxRom::from_ines(ines)?),
        69 => Box::new(Fme7::from_ines(ines)?),
        _ => {
            return Err(format!("Unsupported iNES mapper: {}", ines.header.mapper_number()));
        }
    };

    println!("Successfully loaded mapper: {}", mapper_number);

    return Ok(mapper);
}

pub fn mapper_from_reader(file_reader: &mut dyn Read) -> Result<Box<dyn Mapper>, String> {
    let mut errors = String::new();
    match INesCartridge::from_reader(file_reader) {
        Ok(ines) => {return mapper_from_ines(ines);},
        Err(e) => {errors += format!("ines: {}\n", e).as_str()}
    }

    return Err(format!("Unable to open file as any known type, giving up.\n{}", errors));
}

pub fn mapper_from_file(file_data: &[u8]) -> Result<Box<dyn Mapper>, String> {
    let mut file_reader = file_data;
    return mapper_from_reader(&mut file_reader);
}