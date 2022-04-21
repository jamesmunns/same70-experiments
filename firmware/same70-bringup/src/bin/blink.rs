#![no_main]
#![no_std]

use cortex_m::asm::delay;
use groundhog::RollingTimer;
use same70_bringup::pio::Level;
use same70_bringup::GlobalRollingTimer;
use same70_bringup::{
    self as _,
    efc::Efc,
    hal,
    pio::Pio,
    pmc::{
        ClockSettings, MainClockOscillatorSource, MasterClockSource, MckDivider, MckPrescaler, Pmc,
    },
    wdt::Wdt,
}; // global logger + panicking-behavior + memory layout

#[cortex_m_rt::entry]
fn main() -> ! {
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

    GlobalRollingTimer::init(board.RTT);
    let timer = GlobalRollingTimer::default();

    let mut wdt = Wdt::new(board.WDT);
    wdt.disable();

    let pioa_pins = defmt::unwrap!(Pio::new(board.PIOA, &mut pmc)).split();
    let mut led_pin = pioa_pins.p05.into_push_pull_output(Level::Low);

    defmt::println!("Blankin.");

    let mut ctr = 0;

    loop {
        let start = timer.get_ticks();

        defmt::println!("{=u32}", ctr);
        led_pin.set_low();
        while timer.millis_since(start) <= 250 {}

        led_pin.set_high();
        delay(200_000_000);
        ctr += 1;
        while timer.ticks_since(start) <= 1000 {}
    }
}
