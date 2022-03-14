#![no_main]
#![no_std]

use same70_bringup::{self as _, fixed_setup, hal}; // global logger + panicking-behavior + memory layout
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

    loop {
        let start = timer.get_ticks();

        defmt::println!("{=u32}", ctr);
        board.PIOA.pio_codr.write(|w| {
            // Clear bit
            w.p5().set_bit()
        });
        while timer.millis_since(start) <= 250 { }

        board.PIOA.pio_sodr.write(|w| {
            // set bit
            w.p5().set_bit()
        });
        delay(200_000_000);
        ctr += 1;
        while timer.ticks_since(start) <= 1000 { }
    }
}
