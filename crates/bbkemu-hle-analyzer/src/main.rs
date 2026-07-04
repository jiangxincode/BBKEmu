use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use capstone::prelude::*;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Analyze BBK OS ROM entry points for HLE development")]
struct Cli {
    /// Path to E.BIN.
    rom: PathBuf,

    /// OS bank mapped at logical bank D (e.g. E88 or EA8).
    #[arg(long, default_value = "e88", value_parser = parse_hex_u16)]
    bank_d: u16,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Disassemble a logical ROM address range.
    Disasm {
        #[arg(value_parser = parse_hex_u16)]
        address: u16,
        #[arg(long, default_value_t = 64)]
        length: usize,
    },
    /// List direct JSR targets in a logical ROM address range.
    Calls {
        #[arg(value_parser = parse_hex_u16)]
        address: u16,
        #[arg(long, default_value_t = 4096)]
        length: usize,
    },
    /// Decode three-byte far-call descriptors.
    Descriptors {
        #[arg(required = true, value_parser = parse_hex_u16)]
        addresses: Vec<u16>,
    },
    /// Disassemble a far-call target using the OS segment table base.
    FarDisasm {
        #[arg(value_parser = parse_hex_u8)]
        segment: u8,
        #[arg(value_parser = parse_hex_u16)]
        address: u16,
        #[arg(long, default_value_t = 64)]
        length: usize,
    },
}

fn parse_hex_u16(value: &str) -> std::result::Result<u16, String> {
    let value = value.trim_start_matches("0x");
    u16::from_str_radix(value, 16).map_err(|error| error.to_string())
}

fn parse_hex_u8(value: &str) -> std::result::Result<u8, String> {
    let value = value.trim_start_matches("0x");
    u8::from_str_radix(value, 16).map_err(|error| error.to_string())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let rom =
        fs::read(&cli.rom).with_context(|| format!("failed to read {}", cli.rom.display()))?;
    if rom.len() != 0x20_0000 {
        bail!("expected a 2 MiB E.BIN, got {} bytes", rom.len());
    }

    match cli.command {
        Command::Disasm { address, length } => {
            let bytes = logical_slice(&rom, cli.bank_d, address, length)?;
            let disassembler = create_disassembler()?;
            for instruction in disassembler.disasm_all(bytes, address as u64)?.iter() {
                println!(
                    "{:04X}: {:<9} {:<5} {}",
                    instruction.address(),
                    format_bytes(instruction.bytes()),
                    instruction.mnemonic().unwrap_or(""),
                    instruction.op_str().unwrap_or("")
                );
            }
        }
        Command::Calls { address, length } => {
            let bytes = logical_slice(&rom, cli.bank_d, address, length)?;
            let disassembler = create_disassembler()?;
            let mut calls = BTreeMap::<u16, usize>::new();
            for instruction in disassembler.disasm_all(bytes, address as u64)?.iter() {
                if instruction.mnemonic() == Some("jsr") {
                    if let Some(target) = parse_operand_address(instruction.op_str()) {
                        *calls.entry(target).or_default() += 1;
                    }
                }
            }
            for (target, count) in calls {
                println!("{target:04X} {count}");
            }
        }
        Command::Descriptors { addresses } => {
            for address in addresses {
                let bytes = logical_slice(&rom, cli.bank_d, address, 3)?;
                let target = u16::from_le_bytes([bytes[0], bytes[1]]);
                println!(
                    "{address:04X}: target={target:04X} segment={:02X}",
                    bytes[2]
                );
            }
        }
        Command::FarDisasm {
            segment,
            address,
            length,
        } => {
            let bytes = far_slice(&rom, segment, address, length)?;
            let disassembler = create_disassembler()?;
            for instruction in disassembler.disasm_all(bytes, address as u64)?.iter() {
                println!(
                    "{:04X}: {:<9} {:<5} {}",
                    instruction.address(),
                    format_bytes(instruction.bytes()),
                    instruction.mnemonic().unwrap_or(""),
                    instruction.op_str().unwrap_or("")
                );
            }
        }
    }
    Ok(())
}

fn far_slice(rom: &[u8], segment: u8, address: u16, length: usize) -> Result<&[u8]> {
    if !(0x5000..0x9000).contains(&address) {
        bail!("far-call address must lie in 5000..8FFF");
    }
    if segment >= 0xE0 {
        bail!("flash-backed segments are not stored in E.BIN");
    }
    let base_page = u16::from_le_bytes([rom[0x1F_FFE5], rom[0x1F_FFE4]]);
    let physical_page = base_page as usize + segment as usize * 4;
    let page = (address as usize - 0x5000) >> 12;
    let offset = (physical_page + page - 0x0E00) * 0x1000 + (address as usize & 0x0FFF);
    let end = offset.checked_add(length).context("range overflow")?;
    rom.get(offset..end).context("range lies outside E.BIN")
}

fn create_disassembler() -> Result<Capstone> {
    Capstone::new()
        .mos65xx()
        .mode(capstone::arch::mos65xx::ArchMode::Mos65xx6502)
        .build()
        .context("failed to initialize MOS6502 disassembler")
}

fn logical_slice(rom: &[u8], bank_d: u16, address: u16, length: usize) -> Result<&[u8]> {
    if address < 0xD000 {
        bail!("logical ROM address must be at least D000");
    }
    let logical_page = (address >> 12) - 0x0D;
    let physical_page = bank_d as usize + logical_page as usize;
    let offset = (physical_page - 0x0E00) * 0x1000 + (address as usize & 0x0FFF);
    let end = offset.checked_add(length).context("range overflow")?;
    rom.get(offset..end).context("range lies outside E.BIN")
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_operand_address(operand: Option<&str>) -> Option<u16> {
    let operand = operand?
        .trim()
        .trim_start_matches('$')
        .trim_start_matches("0x");
    u16::from_str_radix(operand, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_logical_address_to_selected_rom_bank() {
        let mut rom = vec![0; 0x20_0000];
        rom[0x88_2F6] = 0xAA;
        assert_eq!(logical_slice(&rom, 0xE88, 0xD2F6, 1).unwrap(), &[0xAA]);
    }

    #[test]
    fn parses_common_operand_formats() {
        assert_eq!(parse_operand_address(Some("$d2f6")), Some(0xD2F6));
        assert_eq!(parse_operand_address(Some("0xD2F6")), Some(0xD2F6));
    }

    #[test]
    fn maps_far_call_segment_using_rom_vector_base() {
        let mut rom = vec![0; 0x20_0000];
        rom[0x1F_FFE4] = 0x0E;
        rom[0x1F_FFE5] = 0x80;
        rom[0xA0_000] = 0xAA;
        assert_eq!(far_slice(&rom, 0x08, 0x5000, 1).unwrap(), &[0xAA]);
    }
}
