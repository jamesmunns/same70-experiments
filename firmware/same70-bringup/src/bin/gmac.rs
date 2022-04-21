#![no_main]
#![no_std]

use core::ops::Deref;

use same70_bringup::efc::Efc;
use same70_bringup::gmac::GmacPins;
use same70_bringup::pio::Pio;
use same70_bringup::pmc::{
    ClockSettings, MainClockOscillatorSource, MasterClockSource, MckDivider, MckPrescaler,
    PeripheralIdentifier, Pmc,
};
use same70_bringup::wdt::Wdt;
use same70_bringup::GlobalRollingTimer;
use same70_bringup::{self as _, gmac::Gmac, hal}; // global logger + panicking-behavior + memory layout

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

    let mut wdt = Wdt::new(board.WDT);
    wdt.disable();

    // TODO: This should *probably* move into HAL methods, once they exist.
    // I'm not sure if any of these are actually used at the moment.
    defmt::unwrap!(pmc.enable_peripherals(&[
        PeripheralIdentifier::TC0_CHANNEL0,
        PeripheralIdentifier::HSMCI,
        PeripheralIdentifier::XDMAC,
    ]));

    let piod_pins = defmt::unwrap!(Pio::new(board.PIOD, &mut pmc)).split();
    let mut port_d_tok = piod_pins.token;

    let mut gmac = defmt::unwrap!(Gmac::new(
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
    ));

    defmt::println!("Polling...");
    let mut ctr: u32 = 0;

    // This loop vaguely manually responds to ARP requests. This was an initial sign of life.
    // You probably want to go look at the `smoltcp.rs` example to see how to use a real
    // tcp stack.
    loop {
        let rf = match gmac.read_frame() {
            Some(f) => {
                let _fsl: &[u8] = f.deref();
                // defmt::println!("Got Frame #{=u32}! Len: {=usize}, Data:", ctr, fsl.len());
                // defmt::println!("{=[u8]:02X}", fsl);
                ctr = ctr.wrapping_add(1);
                f
            }
            None => continue,
        };

        let mut looks_good = true;

        // 00: FF, FF, FF, FF, FF, FF,
        // 06: E8, 6A, 64, 28, 73, 21,
        // 12: 08, 06,
        // 14: 00, 01,
        // 16: 08, 00,
        // 18: 06,
        // 19: 04,
        // 20: 00, 01,
        // 22: E8, 6A, 64, 28, 73, 21,
        // 28: C0, A8, F0, 02,
        // 32: 00, 00, 00, 00, 00, 00,
        // 38: C0, A8, F0, 01,

        looks_good &= &rf[..6] == &[0xFF; 6];
        looks_good &= &rf[12..14] == &[0x08, 0x06];
        looks_good &= &rf[14..16] == &[0x00, 0x01];
        looks_good &= &rf[16..22] == &[0x08, 0x00, 0x06, 0x04, 0x00, 0x01];

        let is_us = &rf[38..42] == &[0xC0, 0xA8, 0xF0, 0x01];

        if !looks_good {
            defmt::println!("Doesn't look ARP-y, skipping...");
            continue;
        }

        if !is_us {
            continue;
        }

        let mut wf = match gmac.alloc_write_frame() {
            Some(wf) => wf,
            None => {
                defmt::println!("Write alloc failed! Skipping response...");
                continue;
            }
        };

        // 00: FF, FF, FF, FF, FF, FF,
        // 06: E8, 6A, 64, 28, 73, 21,
        // 12: 08, 06,
        // 14: 00, 01,
        // 16: 08, 00,
        // 18: 06,
        // 19: 04,
        // 20: 00, 01,
        // 22: E8, 6A, 64, 28, 73, 21,
        // 28: C0, A8, F0, 02,
        // 32: 00, 00, 00, 00, 00, 00,
        // 38: C0, A8, F0, 01,

        // "Our" mac addr: 04-91-62-01-02-03
        wf.iter_mut().for_each(|b| *b = 0);
        wf[0..6].copy_from_slice(&rf[6..12]);
        wf[6..12].copy_from_slice(&[0x04, 0x91, 0x62, 0x01, 0x02, 0x03]);
        wf[12..22].copy_from_slice(&[0x08, 0x06, 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02]);
        wf[22..28].copy_from_slice(&[0x04, 0x91, 0x62, 0x01, 0x02, 0x03]);
        wf[28..32].copy_from_slice(&[0xC0, 0xA8, 0xF0, 0x01]);
        wf[32..38].copy_from_slice(&rf[6..12]);
        wf[38..42].copy_from_slice(&rf[28..32]);
        wf.send(rf.len());
        defmt::println!("Sent!");
    }
}
