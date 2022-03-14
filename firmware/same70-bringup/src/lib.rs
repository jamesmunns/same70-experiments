#![no_main]
#![no_std]

use core::sync::atomic::{AtomicBool, Ordering};

use defmt_rtt as _; // global logger

pub use atsamx7x_hal as hal; // memory layout

use hal::target_device::{Peripherals, WDT, RTT};
use panic_probe as _;

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

/// Terminates the application and makes `probe-run` exit with exit-code = 0
pub fn exit() -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

// defmt-test 0.3.0 has the limitation that this `#[tests]` attribute can only be used
// once within a crate. the module can be in any file but there can only be at most
// one `#[tests]` module in this library crate
#[cfg(test)]
#[defmt_test::tests]
mod unit_tests {
    use defmt::assert;

    #[test]
    fn it_works() {
        assert!(true)
    }
}

defmt::timestamp!("{=u32:us}", {
    GlobalRollingTimer::default().get_ticks().wrapping_mul(1_000_000 / GlobalRollingTimer::TICKS_PER_SECOND)
});

/// Perform fixed, application-specific setup.
pub fn fixed_setup(board: &Peripherals) {
    clock_setup(board);
    disable_watchdog(board);
}

pub fn disable_watchdog(board: &Peripherals) {
    board.WDT.wdt_mr.modify(|_r, w| {
        w.wddis().set_bit()
    });
}

// Set clocks to a CPU frequency of 300MHz, with seven flash wait states,
// and a peripheral frequency of 150MHz.
fn clock_setup(board: &Peripherals) {
    // Note: This is necessary to reach 300MHz operation. Otherwise a hard-fault occurs.
    // We should ALSO probably enable ICACHE to offset the increased wait states, but
    // lets leave it as-is for now...
    defmt::println!("Increasing Flash Wait States to 6...");
    board.EFC.eefc_wpmr.modify(|_r, w| {
        w.wpkey().passwd();
        w.wpen().clear_bit();
        w
    });
    board.EFC.eefc_fmr.modify(|_r, w| {
        unsafe { w.fws().bits(6) }
    });

    defmt::println!("Configuring clocks...");
    // A Main RC oscillator. Three output frequencies can be selected: 4/8/12 MHz. By default 12 MHz is
    // selected. 8 MHz and 12 MHz are factory-trimmed.

    // Note: This follows Datasheet 31.17 "Recommendeded Programming Sequence"
    //
    // Steps 1-5 skipped, using the internal osc
    //
    // # Step 6
    //
    // All parameters needed to configure PLLA and the divider are located in CKGR_PLLAR.
    // CKGR_PLLAR.DIVA is used to control the divider. This parameter can be programmed between 0
    // and 127. Divider output is divider input divided by DIVA parameter. By default, DIVA field is cleared
    // which means that the divider and PLLA are turned off.
    //
    // CKGR_PLLAR.MULA is the PLLA multiplier factor. This parameter can be programmed between 0
    // and 62. If MULA is cleared, PLLA will be turned off, otherwise the PLLA output frequency is PLLA
    // input frequency multiplied by (MULA + 1).
    //
    // CKGR_PLLAR.PLLACOUNT specifies the number of SLCK cycles before PMC_SR.LOCKA is set
    // after CKGR_PLLAR has been written.

    // TODO: Unsure if this is necessary, PLLAR says it needs it
    // NOT to be set.
    //
    // Disable write protection of PMC registers
    board.PMC.pmc_wpmr.modify(|_r, w| {
        w.wpkey().passwd();
        w.wpen().clear_bit();
        w
    });

    board.PMC.ckgr_pllar.modify(|_r, w| {
        w.one().set_bit();
        unsafe {
            // 12 x (24 + 1) => 300MHz
            w.mula().bits(24);

            // This is the reset value?
            w.pllacount().bits(0b111111);

            // 300 / 1 => 300MHz
            w.diva().bits(1);
        }
        w
    });

    // Once CKGR_PLLAR has been written, the user must wait for PMC_SR.LOCKA to be set. This can
    // be done either by polling PMC_SR.LOCKA or by waiting for the interrupt line to be raised if the
    // associated interrupt source (LOCKA) has been enabled in PMC_IER. All fields in CKGR_PLLAR
    // can be programmed in a single write operation. If MULA or DIVA is modified, the LOCKA bit goes
    // low to indicate that PLLA is not yet ready. When PLLA is locked, LOCKA is set again. The user
    // must wait for the LOCKA bit to be set before using the PLLA output clock.
    while board.PMC.pmc_sr.read().locka().bit_is_clear() { }

    // # Step 7
    // Select MCK and HCLK:
    // MCK and HCLK are configurable via PMC_MCKR.
    //
    // CSS is used to select the clock source of MCK and HCLK. By default, the selected clock source is
    // MAINCK.
    //
    // PRES is used to define the HCLK and MCK prescaler.s The user can choose between different
    // values (1, 2, 3, 4, 8, 16, 32, 64). Prescaler output is the selected clock source frequency divided by
    // the PRES value.
    //
    // MDIV is used to define the MCK divider. It is possible to choose between different values (0, 1, 2,
    // 3). MCK output is the HCLK frequency divided by 1, 2, 3 or 4, depending on the value programmed
    // in MDIV.
    //
    // By default, MDIV is cleared, which indicates that the HCLK is equal to MCK.
    // Once the PMC_MCKR has been written, the user must wait for PMC_SR.MCKRDY to be set. This
    // can be done either by polling PMC_SR.MCKRDY or by waiting for the interrupt line to be raised if
    // the associated interrupt source (MCKRDY) has been enabled in PMC_IER. PMC_MCKR must not
    // be programmed in a single write operation. The programming sequence for PMC_MCKR is as
    // follows:
    //
    // If a new value for PMC_MCKR.CSS corresponds to any of the available PLL clocks:
    // a. Program PMC_MCKR.PRES.
    // b. Wait for PMC_SR.MCKRDY to be set.
    board.PMC.pmc_mckr.modify(|_r, w| {
        w.pres().clk_1()
    });
    while board.PMC.pmc_sr.read().mckrdy().bit_is_clear() { }

    // c. Program PMC_MCKR.MDIV.
    // d. Wait for PMC_SR.MCKRDY to be set.
    board.PMC.pmc_mckr.modify(|_r, w| {
        // NOTE: "AT12874" 'Getting Started' guide recommended this,
        // I'm unsure of what the "limit" is, though this now sets
        // our peripheral clock to a base of 150MHz.
        w.mdiv().pck_div2()
    });
    while board.PMC.pmc_sr.read().mckrdy().bit_is_clear() { }

    defmt::println!("Switch PLLA!");

    // e. Program PMC_MCKR.CSS.
    // f. Wait for PMC_SR.MCKRDY to be set.
    board.PMC.pmc_mckr.modify(|_r, w| {
        w.css().plla_clk()
    });
    while board.PMC.pmc_sr.read().mckrdy().bit_is_clear() { }

    // If a new value for PMC_MCKR.CSS corresponds to MAINCK or SLCK:
    // a. Program PMC_MCKR.CSS.
    // b. Wait for PMC_SR.MCKRDY to be set.
    // c. Program PMC_MCKR.PRES.
    // d. Wait for PMC_SR.MCKRDY to be set.
    //
    // If CSS, MDIV or PRES are modified at any stage, the MCKRDY bit goes low to indicate that MCK
    // and HCLK are not yet ready. The user must wait for MCKRDY bit to be set again before using MCK
    // and HCLK.
    //
    // Note: If PLLA clock was selected as MCK and the user decides to modify it by writing a new value
    // into CKGR_PLLAR, the MCKRDY flag will go low while PLLA is unlocked. Once PLLA is locked
    // again, LOCKA goes high and MCKRDY is set.
    //
    // While PLLA is unlocked, MCK selection is automatically changed to SLCK for PLLA. For further
    // information, see "Clock Switching Waveforms".
    //
    // MCK is MAINCK divided by 2.

    // NOTE: Skipping step 9, as we don't need any peripherals (yet).
}

pub fn pet_watchdog() {
    let wdt = unsafe {
        &*WDT::ptr()
    };

    wdt.wdt_cr.write(|w| {
        w.key().passwd();
        w.wdrstt().set_bit();
        w
    });
}

use groundhog::RollingTimer;

static IS_GRT_INIT: AtomicBool = AtomicBool::new(false);
const TICK_SCALER: u32 = 4;

pub struct GlobalRollingTimer {

}

impl Default for GlobalRollingTimer {
    fn default() -> Self {
        GlobalRollingTimer { }
    }
}

impl Clone for GlobalRollingTimer {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl GlobalRollingTimer {
    pub fn init(rtt: RTT) {
        rtt.rtt_mr.modify(|_r, w| {
            // Feed from 32khz prescaled val
            w.rtc1hz().clear_bit();

            // Enable
            w.rttdis().clear_bit();

            // Reset value
            w.rttrst().set_bit();

            // No interrupt
            w.rttincien().clear_bit();

            // No alarm
            w.almien().clear_bit();

            unsafe {
                // Set prescaler to four. We could use three, but four gives us an even
                // number of ticks per second. This gives a minimum resolution of ~122uS,
                // and a maximum range of ~145 hours
                w.rtpres().bits(TICK_SCALER as u16);
            }

            w
        });

        IS_GRT_INIT.store(true, Ordering::SeqCst);
    }
}

impl RollingTimer for GlobalRollingTimer {
    type Tick = u32;

    const TICKS_PER_SECOND: Self::Tick = (32_768 / TICK_SCALER);

    fn get_ticks(&self) -> Self::Tick {
        if !self.is_initialized() {
            return 0;
        }
        let rtt = unsafe { &*RTT::ptr() };

        let mut last = rtt.rtt_vr.read().bits();

        // This value is susceptible to read tearing. Read in a loop
        // to check that values match.
        loop {
            let new = rtt.rtt_vr.read().bits();
            if last == new {
                return last;
            }
            last = new;
        }


    }

    fn is_initialized(&self) -> bool {
        IS_GRT_INIT.load(Ordering::SeqCst)
    }
}
