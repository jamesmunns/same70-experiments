#![no_main]
#![no_std]

use core::sync::atomic::{AtomicBool, Ordering};

use defmt_rtt as _; // global logger

pub use atsamx7x_hal as hal; // memory layout

use hal::target_device::{Peripherals, WDT, RTT};
use panic_probe as _;
pub mod gmac;
pub mod spi;
pub mod pmc;
pub mod efc;

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

defmt::timestamp!("{=f32}", {
    (GlobalRollingTimer::default().get_ticks() as f32) / (32768.0 / 4.0)
});

/// Perform fixed, application-specific setup.
pub fn fixed_setup(board: &Peripherals) {
    clock_setup(board);

    // Refer to Table 14-1 "Peripheral Identifiers" (page 66)
    // for decoding of PIDs used below.
    board.PMC.pmc_pcer0.write(|w| unsafe {
        let mut pcer0 = 0u32;

        pcer0 |= 1 << 23; // PID23 - TC0_CHANNEL0
                          // NOTE: Not actually used!
        pcer0 |= 1 << 21; // PID21 - SPI0
        pcer0 |= 1 << 18; // PID18 - HSMCI
                          // NOTE: Not actually used!
        pcer0 |= 1 << 17; // PID17 - PIOE
        pcer0 |= 1 << 16; // PID16 - PIOD
        pcer0 |= 1 << 14; // PID14 - USART1
                          // NOTE: Not actually used!
        pcer0 |= 1 << 12; // PID12 - PIOC
        pcer0 |= 1 << 11; // PID11 - PIOB
        pcer0 |= 1 << 10; // PID10 - PIOA

        w.bits(pcer0)
    });
    board.PMC.pmc_pcer1.write(|w| unsafe {
        let mut pcer1 = 0u32;

        pcer1 |= 1 << 26; // PID58 - XDMAC
                          // NOTE: Not sure if I actually use this?
        pcer1 |= 1 <<  7; // PID39 - GMAC

        w.bits(pcer1)
    });

    disable_watchdog(board);
    enable_pio_a(board);
    enable_pio_c(board);
    enable_pio_d(board);
}

pub fn disable_watchdog(board: &Peripherals) {
    board.WDT.wdt_mr.modify(|_r, w| {
        w.wddis().set_bit()
    });
}

/// Enable PIO A, map pins
///
/// * map PA05 as an output (it is the LED)
pub fn enable_pio_a(board: &Peripherals) {
    // NOTE: PMC Write protect has already been disabled
    // in `fixed_setup`.

    // Disable PIOA Write protection
    board.PIOA.pio_wpmr.modify(|_r, w| {
        w.wpkey().passwd();
        w.wpen().clear_bit();
        w
    });

//    /************************ PIO A Initialization ************************/
//    /* PORTA Peripheral Function Selection */
//    ((pio_registers_t*)PIO_PORT_A)->PIO_ABCDSR[0]= 0x2000000;
//    ((pio_registers_t*)PIO_PORT_A)->PIO_ABCDSR[1]= 0xde000000;
    board.PIOA.pio_abcdsr[0].write(|w| {
        w.p29().set_bit() // SIGDET
    });
    board.PIOA.pio_abcdsr[1].write(|w| {
        w.p31().set_bit(); // ?
        w.p30().set_bit(); // ?
        w.p28().set_bit(); // ?
        w.p27().set_bit(); // ?
        w.p26().set_bit(); // ?
        w.p25().set_bit(); // ?
        w
    });

//    /* PORTA PIO Disable and Peripheral Enable*/
//    ((pio_registers_t*)PIO_PORT_A)->PIO_PDR = 0xde200000;
    board.PIOA.pio_pdr.write(|w| unsafe { w.bits(0xDE20_0000)});
//    ((pio_registers_t*)PIO_PORT_A)->PIO_PER = ~0xde200000;
    board.PIOA.pio_per.write(|w| unsafe { w.bits(!0xDE20_0000)});
//    ((pio_registers_t*)PIO_PORT_A)->PIO_MDDR = 0xFFFFFFFF;
    board.PIOA.pio_mddr.write(|w| unsafe { w.bits(0xFFFF_FFFF)});

//    /* PORTA Pull Up Enable/Disable as per MHC selection */
//    ((pio_registers_t*)PIO_PORT_A)->PIO_PUDR = ~0x800;
    board.PIOA.pio_pudr.write(|w| unsafe { w.bits(!0x0000_0800)});
//    ((pio_registers_t*)PIO_PORT_A)->PIO_PUER = 0x800;
    board.PIOA.pio_puer.write(|w| unsafe { w.bits(0x0000_0800)});

//    /* PORTA Pull Down Enable/Disable as per MHC selection */
//    ((pio_registers_t*)PIO_PORT_A)->PIO_PPDDR = 0xFFFFFFFF;
    board.PIOA.pio_ppddr.write(|w| unsafe { w.bits(0xFFFF_FFFF)});

//    /* PORTA Output Write Enable */
//    ((pio_registers_t*)PIO_PORT_A)->PIO_OWER = PIO_OWER_Msk;
    board.PIOA.pio_ower.write(|w| unsafe { w.bits(0xFFFF_FFFF)});

//    /* PORTA Output Direction Enable */
//    ((pio_registers_t*)PIO_PORT_A)->PIO_OER = 0x20;
    board.PIOA.pio_oer.write(|w| {
        // Pin PA05, LED, output
        w.p5().set_bit()
    });

//    ((pio_registers_t*)PIO_PORT_A)->PIO_ODR = ~0x20;
    board.PIOA.pio_odr.write(|w| unsafe { w.bits(!0x0000_0020)});

//    /* PORTA Initial state High */
//    ((pio_registers_t*)PIO_PORT_A)->PIO_ODSR = 0x20;
    board.PIOA.pio_odsr.write(|w| {
        w.p5().set_bit()
    });

//    /* PORTA drive control */
//    ((pio_registers_t*)PIO_PORT_A)->PIO_DRIVER = 0x0;
    board.PIOA.pio_driver.write(|w| unsafe { w.bits(0x0000_0000)});

/*
    ### Ethernet Pins

    | Pin     | Name1           | Name2             |
    | :---    | :---            | :---              |
    | PA29    | GPIO            | SIGDET            |
    | PA19    | GPIO            | GPIO0 (N/C?)      |
    | PA02    | WKUP2           | GPIO2             |
*/
}

// enable_pio_b - Not needed yet? Seems to be used mostly for UART.
    // ((pio_registers_t*)PIO_PORT_B)->PIO_ABCDSR[0]= 0x10;
    // ((pio_registers_t*)PIO_PORT_B)->PIO_ABCDSR[1]= 0x10;
    // /* PORTB PIO Disable and Peripheral Enable*/
    // ((pio_registers_t*)PIO_PORT_B)->PIO_PDR = 0x10;
    // ((pio_registers_t*)PIO_PORT_B)->PIO_PER = ~0x10;
    // ((pio_registers_t*)PIO_PORT_B)->PIO_MDDR = 0xFFFFFFFF;
    // /* PORTB Pull Up Enable/Disable as per MHC selection */
    // ((pio_registers_t*)PIO_PORT_B)->PIO_PUDR = 0xFFFFFFFF;
    //  PORTB Pull Down Enable/Disable as per MHC selection
    // ((pio_registers_t*)PIO_PORT_B)->PIO_PPDDR = 0xFFFFFFFF;
    // /* PORTB Output Write Enable */
    // ((pio_registers_t*)PIO_PORT_B)->PIO_OWER = PIO_OWER_Msk;
    // /* PORTB Output Direction Enable */
    // ((pio_registers_t*)PIO_PORT_B)->PIO_OER = 0x100;
    // ((pio_registers_t*)PIO_PORT_B)->PIO_ODR = ~0x100;
    // /* PORTB Initial state High */
    // ((pio_registers_t*)PIO_PORT_B)->PIO_ODSR = 0x100;
    // /* PORTB drive control */
    // ((pio_registers_t*)PIO_PORT_B)->PIO_DRIVER = 0x0;

pub fn enable_pio_c(board: &Peripherals) {
    // NOTE: PMC Write protect has already been disabled
    // in `fixed_setup`.

    // Disable PIOC Write protection
    board.PIOC.pio_wpmr.modify(|_r, w| {
        w.wpkey().passwd();
        w.wpen().clear_bit();
        w
    });

    // /************************ PIO C Initialization ************************/
    // ((pio_registers_t*)PIO_PORT_C)->PIO_PER = 0xFFFFFFFF;
    board.PIOC.pio_per.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // ((pio_registers_t*)PIO_PORT_C)->PIO_MDDR = 0xFFFFFFFF;
    board.PIOC.pio_mddr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTC Pull Up Enable/Disable as per MHC selection */
    // ((pio_registers_t*)PIO_PORT_C)->PIO_PUDR = 0xFFFFFFFF;
    board.PIOC.pio_pudr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTC Pull Down Enable/Disable as per MHC selection */
    // ((pio_registers_t*)PIO_PORT_C)->PIO_PPDDR = 0xFFFFFFFF;
    board.PIOC.pio_ppddr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTC Output Write Enable */
    // ((pio_registers_t*)PIO_PORT_C)->PIO_OWER = PIO_OWER_Msk;
    board.PIOC.pio_ower.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTC Output Direction Enable */
    // ((pio_registers_t*)PIO_PORT_C)->PIO_OER = 0x0;
    board.PIOC.pio_oer.write(|w| unsafe { w.bits(0x0000_0000) });

    // ((pio_registers_t*)PIO_PORT_C)->PIO_ODR = ~0x0;
    board.PIOC.pio_odr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTC Initial state High */
    // ((pio_registers_t*)PIO_PORT_C)->PIO_ODSR = 0x0;
    board.PIOC.pio_odsr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTC drive control */
    // ((pio_registers_t*)PIO_PORT_C)->PIO_DRIVER = 0x0;
    board.PIOC.pio_driver.write(|w| unsafe { w.bits(0x0000_0000) });

/*
    ### Ethernet Pins

    | Pin     | Name1           | Name2             |
    | :---    | :---            | :---              |
    | PC10    | GPIO            | nRST              |
    | PC19    | ISI_PWD         | CS                |
*/
}

pub fn enable_pio_d(board: &Peripherals) {
    // NOTE: PMC Write protect has already been disabled
    // in `fixed_setup`.

    // Disable PIOD Write protection
    board.PIOD.pio_wpmr.modify(|_r, w| {
        w.wpkey().passwd();
        w.wpen().clear_bit();
        w
    });

    let mut periph_pins = 0u32;

    // Ethernet pins
    periph_pins |= 1 << 0; // PD00 - GTXCK
    periph_pins |= 1 << 1; // PD01 - GTXEN
    periph_pins |= 1 << 2; // PD02 - GTX0
    periph_pins |= 1 << 3; // PD03 - GTX1
    periph_pins |= 1 << 4; // PD04 - GRXDV
    periph_pins |= 1 << 5; // PD05 - GRX0
    periph_pins |= 1 << 6; // PD06 - GRX1
    periph_pins |= 1 << 7; // PD07 - GRXER
    periph_pins |= 1 << 8; // PD08 - GMDC
    periph_pins |= 1 << 9; // PD09 - GMDIO

    // SPI0 pins
    periph_pins |= 1 << 20; // PD20 - SPI0_MISO  // ALT B
    periph_pins |= 1 << 21; // PD21 - SPI0_MOSI  // ALT B
    periph_pins |= 1 << 22; // PD22 - SPI0_SPCK  // ALT B
    periph_pins |= 1 << 25; // PD25 - SPI0_NPCS1 // ALT B

    // ALT B means nothing in the top reg, ones in the low reg
    board.PIOD.pio_abcdsr[0].write(|w| {
        w.p20().set_bit();
        w.p21().set_bit();
        w.p22().set_bit();
        w.p25().set_bit();
        w
    });

    // /************************ PIO D Initialization ************************/
    // /* PORTD PIO Disable and Peripheral Enable*/
    // ((pio_registers_t*)PIO_PORT_D)->PIO_PDR = 0x3ff;
    board.PIOD.pio_pdr.write(|w| unsafe { w.bits(periph_pins) });

    // ((pio_registers_t*)PIO_PORT_D)->PIO_PER = ~0x3ff;
    board.PIOD.pio_per.write(|w| unsafe { w.bits(!periph_pins) });

    // ((pio_registers_t*)PIO_PORT_D)->PIO_MDDR = 0xFFFFFFFF;
    board.PIOD.pio_mddr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTD Pull Up Enable/Disable as per MHC selection */
    // ((pio_registers_t*)PIO_PORT_D)->PIO_PUDR = 0xFFFFFFFF;
    board.PIOD.pio_pudr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTD Pull Down Enable/Disable as per MHC selection */
    // ((pio_registers_t*)PIO_PORT_D)->PIO_PPDDR = 0xFFFFFFFF;
    board.PIOD.pio_ppddr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTD Output Write Enable */
    // ((pio_registers_t*)PIO_PORT_D)->PIO_OWER = PIO_OWER_Msk;
    board.PIOD.pio_ower.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTD Output Direction Enable */
    // ((pio_registers_t*)PIO_PORT_D)->PIO_OER = 0x0;
    board.PIOD.pio_oer.write(|w| unsafe { w.bits(0x0000_0000) });

    // ((pio_registers_t*)PIO_PORT_D)->PIO_ODR = ~0x0;
    board.PIOD.pio_odr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

    // /* PORTD Initial state High */
    // ((pio_registers_t*)PIO_PORT_D)->PIO_ODSR = 0x0;
    board.PIOD.pio_odsr.write(|w| unsafe { w.bits(0x0) });

    // /* PORTD drive control */
    // ((pio_registers_t*)PIO_PORT_D)->PIO_DRIVER = 0x0;
    board.PIOD.pio_driver.write(|w| unsafe { w.bits(0x0) });

/*
    ### Ethernet Pins

    | Pin     | Name1           | Name2             |
    | :---    | :---            | :---              |
    | PD00    | GTXCK           | TXCK              |
    | PD01    | GTXEN           | TXEN              |
    | PD02    | GTX0            | TXD0              |
    | PD03    | GTX1            | TXD1              |
    | PD04    | GRXDV           | RXDV              |
    | PD05    | GRX0            | RXD0              |
    | PD06    | GRX1            | RXD1              |
    | PD07    | GRXER           | RXER              |
    | PD08    | GMDC            | MDC               |
    | PD09    | GMDIO           | MDIO              |
    | PD21    | SPI0_MOSI       | MOSI              |
    | PD20    | SPI0_MISO       | MISO              |
    | PD22    | SPI0_SPCK       | SCLK              |
    | PD28    | WKUP5           | GPIO1             |
*/
}

// enable_pio_e - not needed (yet?)
    // /************************ PIO E Initialization ************************/
    // ((pio_registers_t*)PIO_PORT_E)->PIO_PER = 0xFFFFFFFF;
    // ((pio_registers_t*)PIO_PORT_E)->PIO_MDDR = 0xFFFFFFFF;
    // /* PORTE Pull Up Enable/Disable as per MHC selection */
    // ((pio_registers_t*)PIO_PORT_E)->PIO_PUDR = 0xFFFFFFFF;
    // /* PORTE Pull Down Enable/Disable as per MHC selection */
    // ((pio_registers_t*)PIO_PORT_E)->PIO_PPDDR = 0xFFFFFFFF;
    // /* PORTE Output Write Enable */
    // ((pio_registers_t*)PIO_PORT_E)->PIO_OWER = PIO_OWER_Msk;
    // /* PORTE Output Direction Enable */
    // ((pio_registers_t*)PIO_PORT_E)->PIO_OER = 0x0;
    // ((pio_registers_t*)PIO_PORT_E)->PIO_ODR = ~0x0;
    // /* PORTE Initial state High */
    // ((pio_registers_t*)PIO_PORT_E)->PIO_ODSR = 0x0;
    // /* PORTE drive control */
    // ((pio_registers_t*)PIO_PORT_E)->PIO_DRIVER = 0x0;

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
    // PRES is used to define the HCLK and MCK prescalers The user can choose between different
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
