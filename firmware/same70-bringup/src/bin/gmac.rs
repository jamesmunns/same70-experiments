#![no_main]
#![no_std]

use same70_bringup::{self as _, fixed_setup, hal, gmac::Gmac}; // global logger + panicking-behavior + memory layout
use cortex_m::asm::delay;
use groundhog::RollingTimer;
use same70_bringup::GlobalRollingTimer;

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Blink!");

    // Obtain PAC-level access
    let board = hal::target_device::Peripherals::take().unwrap();

    // Setup with general purpose settings
    fixed_setup(&board);
    GlobalRollingTimer::init(board.RTT);
    let timer = GlobalRollingTimer::default();

    defmt::println!("Blankin.");

    defmt::println!("Creating GMAC...");
    let mut gmac = unsafe { Gmac::new(board.GMAC) };

    defmt::println!("Initializing...");
    gmac.init();

    defmt::println!("Polling...");
    loop {
        if gmac.did_it_work() {
            defmt::println!("Worked!");
            same70_bringup::exit();
        }
    }
}
