#![no_main]
#![no_std]

use core::ops::Deref;

use same70_bringup::{self as _, fixed_setup, hal, gmac::Gmac}; // global logger + panicking-behavior + memory layout
use same70_bringup::GlobalRollingTimer;

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Blink!");

    // Obtain PAC-level access
    let board = hal::target_device::Peripherals::take().unwrap();

    // Setup with general purpose settings
    fixed_setup(&board);
    GlobalRollingTimer::init(board.RTT);
    // let timer = GlobalRollingTimer::default();

    defmt::println!("Blankin.");

    defmt::println!("Creating GMAC...");
    let mut gmac = unsafe { Gmac::new(board.GMAC) };

    defmt::println!("Initializing...");
    gmac.init();

    defmt::println!("MIIM setup...");
    gmac.miim_post_setup();

    // same70_bringup::exit();

    defmt::println!("Polling...");
    let mut ctr = 0;

    loop {
        match gmac.read_frame() {
            Some(f) => {
                let fsl: &[u8] = f.deref();
                defmt::println!("Got Frame #{=u32}! Len: {=usize}, Data:", ctr, fsl.len());
                defmt::println!("{=[u8]:02X}", fsl);
                ctr = ctr.wrapping_add(1);
            }
            None => {},
        }
    }
}
