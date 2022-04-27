/*

SPI Locations

* J503 - 6 pin SPI Header
    * SPI0_SPCK  - PD22
    * SPI0_MISO  - PD20
    * SPI0_MOSI  - PD21
    * NOTE: No CSn pin!
* J901 - MikroBUS
    * SPI0_SPCK  - PD22
    * SPI0_MISO  - PD20
    * SPI0_MOSI  - PD21
    * CS         - PA05
* J602 - **EXT1**
    * SPI0_SPCK  - PD22
    * SPI0_MISO  - PD20
    * SPI0_MOSI  - PD21
    * SPI0_NPCS1 - PD25
* J604 - EXT2
    * SPI0_SPCK  - PD22
    * SPI0_MISO  - PD20
    * SPI0_MOSI  - PD21
    * SPI0_NPCS3 - PD27
* Arduino Header

Also brought out to the Ethernet header? Are we reusing any pins?
Doesn't look like it:

>>> hex((1 << 21) | (1 << 20) | (1 << 22))
'0x700000'

// ((pio_registers_t*)PIO_PORT_D)->PIO_PER = ~0x3ff;
board.PIOD.pio_per.write(|w| unsafe { w.bits(!0x0000_03FF) });

*/

use core::num::NonZeroU8;

use groundhog::RollingTimer;

use crate::GlobalRollingTimer;
use crate::target_device::{PIOD, SPI0};

use crate::{
    pio::{PeriphB, Pin},
    pmc::{PeripheralIdentifier, Pmc},
};

// This could be made generic, but hasn't yet been.
pub struct Spi0Pins {
    pub miso: Pin<PIOD, PeriphB, 20>,
    pub mosi: Pin<PIOD, PeriphB, 21>,
    pub spck: Pin<PIOD, PeriphB, 22>,
    // TODO: npcs0
    pub npcs1: Pin<PIOD, PeriphB, 25>,
    // TODO: npcs2
    // TODO: npcs3
}

// This could be made generic, but hasn't yet been.
pub struct Spi0 {
    periph: SPI0,
    _current_freq: SpiFreq,
    _pins: Spi0Pins,
}

/// A collection of common SPI Frequencies.
pub enum SpiFreq {
    /// 150.0 MHz
    M150_0,
    /// 75.0 MHz
    M75_0,
    /// 50.0 MHz
    M50_0,
    /// 37.5 MHz
    M37_5,
    /// 30.0 MHz
    M30_0,
    /// 25.0 MHz
    M25_0,
    /// 15.0 MHz
    M15_0,
    /// 12.5 MHz
    M12_5,
    /// 10.0 MHz
    M10_0,
    /// 7.5 MHz
    M7_5,
    /// 6.0 MHz
    M6_0,
    /// 5.0 MHz
    M5_0,
    /// 3.0 MHz
    M3_0,
    /// 2.5 MHz
    M2_5,
    ///.2 MHz
    M2,
    /// 1.5 MHz
    M1_5,
    /// 1.0 MHz
    M1_0,
    /// 0.6 MHz
    M0_6,
    /// A custom divisor. Must be nonzero. The resulting
    /// frequency is (150MHz / Custom).
    Custom(NonZeroU8),
}

impl SpiFreq {
    pub fn to_baud_divisor(&self) -> u8 {
        // NOTE: These all assume a peripheral clock of
        // 150MHz! If this assumption changes, these MUST be
        // updated!
        match self {
            SpiFreq::M150_0 => 1,
            SpiFreq::M75_0 => 2,
            SpiFreq::M50_0 => 3,
            SpiFreq::M37_5 => 4,
            SpiFreq::M30_0 => 5,
            SpiFreq::M25_0 => 6,
            SpiFreq::M15_0 => 10,
            SpiFreq::M12_5 => 12,
            SpiFreq::M10_0 => 15,
            SpiFreq::M7_5 => 20,
            SpiFreq::M6_0 => 25,
            SpiFreq::M5_0 => 30,
            SpiFreq::M3_0 => 50,
            SpiFreq::M2_5 => 60,
            SpiFreq::M2 => 75,
            SpiFreq::M1_5 => 100,
            SpiFreq::M1_0 => 150,
            SpiFreq::M0_6 => 250,
            SpiFreq::Custom(f) => f.get(),
        }
    }
}

pub enum SelectedTarget {
    Target0,
    Target1,
    Target2,
    Target3,
}

impl SelectedTarget {
    pub fn to_pcs(&self) -> u8 {
        match self {
            SelectedTarget::Target0 => 0b1110,
            SelectedTarget::Target1 => 0b1101,
            SelectedTarget::Target2 => 0b1011,
            SelectedTarget::Target3 => 0b0111,
        }
    }
}

impl Spi0 {
    // TODO: Always gives you an 8-bit, MODE0, SPI port.
    pub fn new(
        spi0: SPI0,
        initial_freq: SpiFreq,
        pins: Spi0Pins,
        pmc: &mut Pmc,
    ) -> Result<Self, ()> {
        // TODO: For now all the "baud divisor" math assumes an MCLK of
        // 150MHz. Update that code before removing this check!
        defmt::println!("Check baud...");
        {
            let settings = pmc.settings().ok_or(())?;
            defmt::println!("Got settings...");
            let mclk = settings.calc_master_clk_mhz().map_err(drop)?;
            defmt::println!("Baud: {=u8}", mclk);
            let timer = GlobalRollingTimer::default();
            let start = timer.get_ticks();
            while timer.millis_since(start) < 500 {}
            if mclk != 150 {
                defmt::println!("mclk: {=u8}", mclk);
                return Err(());
            }
        }

        defmt::println!("Enable Periph...");

        // Enable the SPI0 peripheral
        pmc.enable_peripherals(&[PeripheralIdentifier::SPI0])
            .map_err(drop)?;

        spi0.spi_mr.write(|w| {
            // No delay between chip select switches
            unsafe {
                w.dlybcs().bits(0);
            }
            // Start with npcs1 selected
            w.pcs().npcs1();
            // Local Loopback disabled
            w.llb().clear_bit();
            // Wait for data read before transfer start
            w.wdrbt().set_bit();
            // Disable mode fault detection
            w.modfdis().set_bit();
            // Use direct chip selects, not via a decoder
            w.pcsdec().clear_bit();
            // Allow the hardware to handle chip select
            w.ps().set_bit();
            // Set to master/controller mode
            w.mstr().set_bit();
            w
        });

        // Read and clear any status flags
        let _ = spi0.spi_sr.read();

        // No interrupts for now
        spi0.spi_idr.write(|w| {
            w.undes().set_bit();
            w.txempty().set_bit();
            w.nssr().set_bit();
            w.ovres().set_bit();
            w.modf().set_bit();
            w.tdre().set_bit();
            w.rdrf().set_bit();
            w
        });

        // For now, configure all target devices using the same config.
        for csr in spi0.spi_csr.iter() {
            csr.write(|w| {
                unsafe {
                    // Delay between consecutive transfers
                    w.dlybct().bits(0);
                    // delay before spck
                    w.dlybs().bits(0);
                    // serial clock bit rate
                    w.scbr().bits(initial_freq.to_baud_divisor());
                    // Hardcoded to 8 bit words for now
                    w.bits_()._8_bit();
                    // Don't auto-clear chip select
                    // (we also enable MR::PS which allows for dynamic
                    // chip select based on end of transmission)
                    w.csaat().set_bit();
                    // Ignored when CSAAT active
                    w.csnaat().clear_bit();
                    // Hardcoded to "Mode 0"
                    w.ncpha().valid_leading_edge(); // NCPHA: 1/CPHA: 0
                    w.cpol().idle_low(); // CPOL: 0
                }

                w
            });
        }

        // Enable the device
        spi0.spi_cr.write(|w| {
            w.spien().set_bit();
            w
        });

        defmt::println!("Done!");

        Ok(Self {
            periph: spi0,
            _current_freq: initial_freq,
            _pins: pins,
        })
    }


    // out: &[0x01, 0x02, 0x00, 0x00, 0x00, 0x00]
    // in:  &mut [0x00, 0x00, 0x00, 0x00, 0x00, 0x00]

    // out: &[0x01, 0x02, 0x00, 0x00, 0x00, 0x00]
    // in:  &mut [0x00, 0x00, 0x10, 0x20, 0x30, 0x40]

    // TODO (AJM): Add API for changing SPI frequency scaler
    pub fn transfer_basic(
        &mut self,
        target: SelectedTarget,
        txmt: &[u8],
        recv: &mut [u8],
    ) -> Result<(), ()> {
        // Basic length checks...
        if recv.len() != txmt.len() {
            return Err(());
        }

        struct TxWord {
            last: bool,
            data: u8,
        }

        // For all but the last byte...
        let tgt_pcs = target.to_pcs();
        // If there are zero items, return an error.
        let (last, rest) = txmt.split_last().ok_or(())?;

        let mut tx_iter = rest
            .iter()
            .map(|w| TxWord {
                last: false,
                data: *w,
            })
            .chain(core::iter::once(TxWord {
                last: true,
                data: *last,
            }));

        let mut rx_iter = recv.iter_mut().peekable();
        let mut tx_done = false;
        let mut rx_done = false;

        // Read (and clear) any status/error flags
        let status = self.periph.spi_sr.read();
        assert!(status.spiens().bit_is_set(), "[SPI] Not enabled?");
        assert!(status.ovres().bit_is_clear(), "[SPI] Read Overrun!");

        // Work until transmit is empty and receive is full
        while !(tx_done && rx_done) {
            let status = self.periph.spi_sr.read();

            // Are we ready to send data?
            if !tx_done && status.tdre().bit_is_set() {
                // Is there any data left to send?
                if let Some(dat) = tx_iter.next() {
                    self.periph.spi_tdr.write(|w| unsafe {
                        w.lastxfer().bit(dat.last);
                        w.pcs().bits(tgt_pcs);
                        w.td().bits(dat.data.into());
                        w
                    });
                } else {
                    // Nope! All done sending.
                    tx_done = true;
                }
            }

            if !rx_done {
                // Is there any data left to receive?
                if rx_iter.peek().is_none() {
                    // Nope! All done receiving.
                    rx_done = true;
                } else {
                    // Has data been received?
                    if status.rdrf().bit_is_set() {
                        // Yes! Store the data
                        if let Some(dat) = rx_iter.next() {
                            *dat = self.periph.spi_rdr.read().bits() as u8;
                        } else {
                            // Should never happen: We peeked and it was there!
                        }
                    }
                }
            }
        }

        defmt::println!("Spi done!");

        Ok(())
    }
}
