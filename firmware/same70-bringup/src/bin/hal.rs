#![no_main]
#![no_std]

use groundhog::RollingTimer;
use same70_bringup::gmac::{Gmac, GmacPins};
use same70_bringup::wdt::Wdt;
use same70_bringup::GlobalRollingTimer;
use same70_bringup::{
    self as _,
    efc::Efc,
    hal,
    pio::Pio,
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

    GlobalRollingTimer::init(board.RTT);
    let timer = GlobalRollingTimer::default();

    let mut wdt = Wdt::new(board.WDT);
    wdt.disable();

    // TODO: This should *probably* move into HAL methods, once they exist.
    // I'm not sure if any of these are actually used at the moment.
    defmt::unwrap!(pmc.enable_peripherals(&[
        PeripheralIdentifier::TC0_CHANNEL0,
        PeripheralIdentifier::HSMCI,
        PeripheralIdentifier::XDMAC,
    ]));

    let _pioa_pins = defmt::unwrap!(Pio::new(board.PIOA, &mut pmc)).split();
    let _piob_pins = defmt::unwrap!(Pio::new(board.PIOB, &mut pmc)).split();
    let _pioc_pins = defmt::unwrap!(Pio::new(board.PIOC, &mut pmc)).split();
    let piod_pins = defmt::unwrap!(Pio::new(board.PIOD, &mut pmc)).split();
    let _pioe_pins = defmt::unwrap!(Pio::new(board.PIOE, &mut pmc)).split();

    let mut port_d_tok = piod_pins.token;

    let _gmac = Gmac::new(
        board.GMAC,
        GmacPins {
            gtxck: piod_pins.p00.into_periph_mode_a(&mut port_d_tok),
            gtxen: piod_pins.p01.into_periph_mode_a(&mut port_d_tok),
            gtx0: piod_pins.p02.into_periph_mode_a(&mut port_d_tok),
            gtx1: piod_pins.p03.into_periph_mode_a(&mut port_d_tok),
            grxdv: piod_pins.p04.into_periph_mode_a(&mut port_d_tok),
            grx0: piod_pins.p05.into_periph_mode_a(&mut port_d_tok),
            grx1: piod_pins.p06.into_periph_mode_a(&mut port_d_tok),
            grxer: piod_pins.p07.into_periph_mode_a(&mut port_d_tok),
            gmdc: piod_pins.p08.into_periph_mode_a(&mut port_d_tok),
            gmdio: piod_pins.p09.into_periph_mode_a(&mut port_d_tok),
        },
        &mut pmc,
        // 04:91:62:01:02:03
        [0x04, 0x91, 0x62, 0x01, 0x02, 0x03],
    );

    defmt::println!("Blankin.");

    // I really only am using this to make sure the HAL code compiles.
    loop {
        let start = timer.get_ticks();
        while timer.ticks_since(start) <= 1000 {}
    }
}
