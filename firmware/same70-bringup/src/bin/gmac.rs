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
    let mut ctr: u32 = 0;

    loop {
        let rf = match gmac.read_frame() {
            Some(f) => {
                let fsl: &[u8] = f.deref();
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
