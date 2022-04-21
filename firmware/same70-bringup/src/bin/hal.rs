#![no_main]
#![no_std]

use cortex_m::asm::delay;
use groundhog::RollingTimer;
use same70_bringup::GlobalRollingTimer;
use same70_bringup::{
    self as _,
    efc::Efc,
    hal,
    pmc::{
        ClockSettings, MainClockOscillatorSource, MasterClockSource, MckDivider, MckPrescaler,
        PeripheralIdentifier, Pmc,
    },
}; // global logger + panicking-behavior + memory layout

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Blink!");

    // Obtain PAC-level access
    let board = hal::target_device::Peripherals::take().unwrap();

    let mut efc = Efc::new(board.EFC);
    let mut pmc = Pmc::new(board.PMC);

    let clk_cfg = ClockSettings {
        main_clk_osc_src: MainClockOscillatorSource::MainCk12MHz,
        mck_pres: MckPrescaler::CLK_1,
        mck_src: MasterClockSource::PllaClock,
        mck_div: MckDivider::PCK_DIV2, // 300MHz / 2 = 150MHz
        multiplier_a: 24,              // (24 + 1) * 12 = 300MHz
        divider_a: 1,                  // 300MHz / 1 = 300MHz
    };

    defmt::unwrap!(pmc.set_clocks(&mut efc, clk_cfg));

    // TODO: This should *probably* move into HAL methods, e.g. this
    // gets done in PioA::new(board.PIOA, &mut pmc);
    defmt::unwrap!(pmc.enable_peripherals(&[
        PeripheralIdentifier::TC0_CHANNEL0,
        PeripheralIdentifier::SPI0,
        PeripheralIdentifier::HSMCI,
        PeripheralIdentifier::PIOA,
        PeripheralIdentifier::PIOB,
        PeripheralIdentifier::PIOC,
        PeripheralIdentifier::PIOD,
        PeripheralIdentifier::PIOE,
        PeripheralIdentifier::XDMAC,
        PeripheralIdentifier::GMAC,
    ]));

    GlobalRollingTimer::init(board.RTT);
    let timer = GlobalRollingTimer::default();

    defmt::println!("Blankin.");

    let mut ctr = 0;

    loop {
        let start = timer.get_ticks();

        defmt::println!("{=u32}", ctr);
        board.PIOA.pio_codr.write(|w| {
            // Clear bit
            w.p5().set_bit()
        });
        while timer.millis_since(start) <= 250 {}

        board.PIOA.pio_sodr.write(|w| {
            // set bit
            w.p5().set_bit()
        });
        delay(200_000_000);
        ctr += 1;
        while timer.ticks_since(start) <= 1000 {}
    }
}
