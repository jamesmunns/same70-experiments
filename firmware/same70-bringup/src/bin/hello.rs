#![no_main]
#![no_std]

use same70_bringup as _; // global logger + panicking-behavior + memory layout

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Hello, world!");

    same70_bringup::exit()
}
