#![no_main]
#![no_std]

use same70_bringup::{self as _, fixed_setup, hal}; // global logger + panicking-behavior + memory layout
use cortex_m::asm::delay;

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Blink!");

    // Obtain PAC-level access
    let board = hal::target_device::Peripherals::take().unwrap();

    // Setup with general purpose settings
    fixed_setup(&board);

    defmt::println!("Blankin.");

    // NOTE: PMC Write protect has already been disabled
    // in `fixed_setup`.

    // Enable PIOA
    board.PMC.pmc_pcer0.write(|w| {
        w.pid10().set_bit()
    });

    // Disable PIO Write protection
    board.PIOA.pio_wpmr.modify(|_r, w| {
        w.wpkey().passwd();
        w.wpen().clear_bit();
        w
    });

    board.PIOA.pio_per.write(|w| {
        // Pin PA05, LED, enabled
        w.p5().set_bit()
    });

    board.PIOA.pio_oer.write(|w| {
        // Pin PA05, LED, output
        w.p5().set_bit()
    });

    defmt::println!("Loopin!");

    let mut ctr = 0;

    let val = board.WDT.wdt_mr.read().bits();
    defmt::println!("{=u32:08X}", val);

    loop {
        defmt::println!("{=u32}", ctr);
        board.PIOA.pio_codr.write(|w| {
            // Clear bit
            w.p5().set_bit()
        });
        delay(100_000_000);
        board.PIOA.pio_sodr.write(|w| {
            // set bit
            w.p5().set_bit()
        });
        delay(200_000_000);
        ctr += 1;
    }
}


// WDT MR: 3FFF_2FFF
// enabled
// does reset
// no interrupt
