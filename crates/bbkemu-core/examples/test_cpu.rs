use bbkemu_core::memory::Memory;
use bbkemu_core::cpu::CpuWrapper;

fn main() {
    println!("Creating memory...");
    let memory = Memory::new();
    println!("Memory created, size: {}", std::mem::size_of::<Memory>());

    println!("Creating CPU...");
    let cpu = CpuWrapper::new(memory);
    println!("CPU created, size: {}", std::mem::size_of::<CpuWrapper>());

    println!("PC: 0x{:04X}", cpu.pc());
    println!("A: 0x{:02X}", cpu.a());
}
