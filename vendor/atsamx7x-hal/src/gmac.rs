//! GMAC Ethernet Peripheral
//!
//! At the moment, this module has limited support to allow for basic
//! ethernet functionality, with integration for the `smoltcp` TCP/IP
//! network stack

use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull, sync::atomic::{fence, Ordering, compiler_fence},
};

use crate::target_device::{GMAC, PIOD};
use groundhog::RollingTimer;
use smoltcp::phy::{
    Checksum, ChecksumCapabilities, Device, DeviceCapabilities, Medium, RxToken, TxToken,
};

use crate::{
    pio::{PeriphA, Pin},
    pmc::{PeripheralIdentifier, Pmc},
    GlobalRollingTimer,
};

const NUM_RX_BUFS: usize = 4;
const NUM_TX_BUFS: usize = 4;
const RX_BUF_SIZE: usize = 1024;
const TX_BUF_SIZE: usize = 1024;

const RX_BUF_DEFAULT: RxBuffer = RxBuffer {
    buf: UnsafeCell::new([0u8; RX_BUF_SIZE]),
};
const TX_BUF_DEFAULT: TxBuffer = TxBuffer {
    buf: UnsafeCell::new([0u8; TX_BUF_SIZE]),
};

const RX_BUF_DESC_DEFAULT: RxBufferDescriptor = RxBufferDescriptor {
    words: UnsafeCell::new([0u32; 2]),
};

const TX_BUF_DESC_DEFAULT: TxBufferDescriptor = TxBufferDescriptor {
    words: UnsafeCell::new([0u32; 2]),
};

// TODO: This needs a specific linker section (probably)
// Todo: UnsafeCell?
static RX_BUF_DESCS: [RxBufferDescriptor; NUM_RX_BUFS] = [RX_BUF_DESC_DEFAULT; NUM_RX_BUFS];
static RX_BUFS: [RxBuffer; NUM_RX_BUFS] = [RX_BUF_DEFAULT; NUM_RX_BUFS];
static TX_BUF_DESCS: [TxBufferDescriptor; NUM_TX_BUFS] = [TX_BUF_DESC_DEFAULT; NUM_TX_BUFS];
static TX_BUFS: [TxBuffer; NUM_TX_BUFS] = [TX_BUF_DEFAULT; NUM_TX_BUFS];

static UNUSED_TX_BUF_DESC: TxBufferDescriptor = TX_BUF_DESC_DEFAULT;

/// Pins mapped to the GMAC peripheral functionality
//
//. TODO: This should probably be replaced with a struct with a lot of generics,
//. or some other way. For now, support a single fixed pin mapping option
//. (PRs welcome!)
pub struct GmacPins {
    pub gtxck: Pin<PIOD, PeriphA, 00>,
    pub gtxen: Pin<PIOD, PeriphA, 01>,
    pub gtx0: Pin<PIOD, PeriphA, 02>,
    pub gtx1: Pin<PIOD, PeriphA, 03>,
    pub grxdv: Pin<PIOD, PeriphA, 04>,
    pub grx0: Pin<PIOD, PeriphA, 05>,
    pub grx1: Pin<PIOD, PeriphA, 06>,
    pub grxer: Pin<PIOD, PeriphA, 07>,
    pub gmdc: Pin<PIOD, PeriphA, 08>,
    pub gmdio: Pin<PIOD, PeriphA, 09>,
}

/// A smoltcp token representing a received ethernet frame
pub struct GmacRxToken<'a> {
    rf: ReadFrame,
    _plt: PhantomData<&'a ()>,
}

impl<'a> RxToken for GmacRxToken<'a> {
    fn consume<R, F>(mut self, _timestamp: smoltcp::time::Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        defmt::trace!("[GMAC]: RX: {=[u8]:02X}", &self.rf);
        f(self.rf.deref_mut())
    }
}


/// A smoltcp token representing the capability to send an
/// ethernet frame
pub struct GmacTxToken<'a> {
    gmac: &'a mut Gmac,
}

impl<'a> TxToken for GmacTxToken<'a> {
    fn consume<R, F>(
        self,
        _timestamp: smoltcp::time::Instant,
        len: usize,
        f: F,
    ) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut tf = self.gmac.alloc_write_frame().unwrap();
        let res = f(&mut tf[..len]);
        defmt::trace!("TX: {=[u8]:02X}", &tf[..len]);
        tf.send(len);
        defmt::info!("[GMAC] SENT FRAME");
        res
    }
}

/// An implemntation of the smotcp `Device` trait. This allows us
/// to use the GMAC struct with the smoltcp stack.
impl<'a> Device<'a> for Gmac {
    type RxToken = GmacRxToken<'a>;
    type TxToken = GmacTxToken<'a>;

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let rxf = self.read_frame()?;
        defmt::println!("[GMAC] GOT FRAME");
        Some((
            GmacRxToken {
                rf: rxf,
                _plt: PhantomData,
            },
            GmacTxToken { gmac: self },
        ))
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(GmacTxToken { gmac: self })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capa = DeviceCapabilities::default();
        capa.medium = Medium::Ethernet;
        capa.max_transmission_unit = 1024; // Is this too big?
        capa.max_burst_size = None;

        let mut cksm = ChecksumCapabilities::ignored();
        cksm.ipv4 = Checksum::None;
        cksm.tcp = Checksum::None;
        cksm.udp = Checksum::None;
        cksm.icmpv4 = Checksum::Tx;

        capa.checksum = cksm;
        capa
    }
}

/// The GMAC peripheral HAL interface
///
/// This interface also implements the smoltcp Device trait, allowing
/// it to be used with the smoltcp TCP/IP stack.
pub struct Gmac {
    periph: GMAC,
    next_tx_idx: usize,
    last_txgo: bool,
    last_bna: bool,
    last_stat_poll: u32,
    _pins: GmacPins,
    mac_addr: [u8; 6],
}

/// A received ethernet frame
///
/// This represents a position in the hardware-based frame buffer. It should
/// be used and dropped as soon as reasonably possible, in order to free up
/// space to receive additional frames.
pub struct ReadFrame {
    bufr: NonNull<[u8; RX_BUF_SIZE]>,
    len: usize,
    desc: NonNull<RxBufferDescriptor>,
}

impl Deref for ReadFrame {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.bufr.as_ptr().cast(), self.len) }
    }
}

impl DerefMut for ReadFrame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.bufr.as_ptr().cast(), self.len) }
    }
}

impl Drop for ReadFrame {
    fn drop(&mut self) {
        // On drop, we must reset the header to "free" it.
        let desc = unsafe { self.desc.as_ref() };
        // Get w0 to figure out if this is the "last" item
        // TODO: Just check against buffer address?
        let is_last = (desc.get_word_0() & 0x0000_0002) != 0;
        let buf_addr = self.bufr.as_ptr();
        let buf_word = buf_addr as u32;
        let buf_word_msk = buf_word & 0xFFFF_FFFC;

        let last_word = if is_last { 0x0000_0002 } else { 0x0000_0000 };

        // defmt::println!("Releasing ReadFrame @ {=u32:08X}", buf_word_msk);

        // Note, bit 0 is ALWAYS cleared here, which marks the buffer as ready for
        // re-use by the GMAC
        desc.set_word_0(buf_word_msk | last_word);
    }
}

/// A to-be-sent ethernet frame
///
/// This represents a position in the hardware-based outgoing frame buffer.
/// Once filled, and `send()` is called, it will be made available to the hardware
/// to be sent. It should be sent as quickly as reasonably possible, in order to
/// avoid stalling the outgoing TCP frames.
pub struct WriteFrame {
    bufr: NonNull<[u8; TX_BUF_SIZE]>,
    desc: NonNull<TxBufferDescriptor>,
    was_sent: bool,
}

impl Drop for WriteFrame {
    fn drop(&mut self) {
        if !self.was_sent {
            defmt::println!("Dropping unsent frame!");
            unsafe { self.dropper(0) }
        }
    }
}

impl Deref for WriteFrame {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.bufr.as_ptr().cast(), TX_BUF_SIZE) }
    }
}

impl DerefMut for WriteFrame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.bufr.as_ptr().cast(), TX_BUF_SIZE) }
    }
}

impl WriteFrame {
    unsafe fn dropper(&mut self, len: usize) {
        compiler_fence(Ordering::SeqCst);

        let desc = { self.desc.as_ref() };
        let old_w1 = desc.get_word_1();
        let wrap_bit = old_w1 & 0x4000_0000;
        let len = len.min(TX_BUF_SIZE).min(0x3FFF) as u32;

        let mut new_w1 = 0;
        // Bit 31 is zeroed to mark this as "ready"
        // Bit 30: Wrap
        new_w1 |= wrap_bit;
        // Bits 29:17 are status/reserved bits, okay to clear
        // Bit 16 is zeroed to have CRC calc offloaded
        // Bit 15: Last Buffer in Frame
        new_w1 |= 0x0000_8000;
        // Bit 14 is reserved
        // Bits 13:0: Size
        new_w1 |= len;

        // Store the word to make it active for the transmit hardware to process.
        desc.set_word_1(new_w1);
        self.was_sent = true;

        fence(Ordering::SeqCst);

        // Conjure the GMAC peripheral so we can trigger a send.

        {
            let gmac = &*GMAC::ptr();
            gmac.gmac_ncr.modify(|_r, w| w.tstart().set_bit());
        }
    }

    pub fn send(mut self, len: usize) {
        unsafe { self.dropper(len) };
    }
}

impl Gmac {
    /// Create a new HAL representation of the GMAC peripheral.
    ///
    /// Requires the necessary pins to be mapped in the correct mode. It will
    /// enable the necessary PMC clocks automatically.
    ///
    /// Also takes a MAC address in the form of a 6 byte array. For example:
    ///
    /// `[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]` would map to the MAC address:
    /// `01:02:03:04:05:06` in typical notation.
    pub fn new(periph: GMAC, pins: GmacPins, pmc: &mut Pmc, mac_addr: [u8; 6]) -> Result<Self, ()> {
        // Enable the gmac peripheral
        pmc.enable_peripherals(&[PeripheralIdentifier::GMAC])
            .map_err(drop)?;
        let timer = GlobalRollingTimer::default();

        // Initial configuration
        let mut gmac = Self {
            periph,
            _pins: pins,
            next_tx_idx: 0,
            last_txgo: false,
            last_bna: false,
            last_stat_poll: timer.get_ticks(),
            mac_addr,
        };
        gmac.init();
        gmac.miim_post_setup();

        Ok(gmac)
    }

    /// Obtain a copy of the configured MAC address
    pub fn mac_addr(&self) -> [u8; 6] {
        self.mac_addr.clone()
    }

    /// Query the relevant status and statistics registers, logging them with `defmt`.
    ///
    /// Also clears any active flags.
    pub fn query(&mut self) {
        // Query TSR
        self.periph.gmac_tsr.modify(|r, w| {
            if r.hresp().bit_is_set() {
                defmt::error!("[TSR]: HRESP Not OK");
                w.hresp().set_bit();
            }

            if r.txcomp().bit_is_set() {
                defmt::info!("[TSR]: Frame Transmit Complete");
                w.txcomp().set_bit();
            }

            if r.tfc().bit_is_set() {
                defmt::error!("[TSR]: Transmit Frame Corruption Due to AHB Error");
                w.tfc().set_bit();
            }

            let txgo = r.txgo().bit_is_set();
            if txgo != self.last_txgo {
                self.last_txgo = txgo;
                defmt::info!("[TSR]: TXGO changed to {=bool}", txgo);
            }

            if r.rle().bit_is_set() {
                defmt::warn!("[TSR]: Retry Limit Exceeded");
                w.rle().set_bit();
            }

            if r.col().bit_is_set() {
                defmt::warn!("[TSR]: Collision Occurred");
                w.col().set_bit();
            }

            if r.ubr().bit_is_set() {
                w.ubr().set_bit();
            }

            w
        });

        // Query RSR
        self.periph.gmac_rsr.modify(|r, w| {
            if r.hno().bit_is_set() {
                defmt::error!("[RSR]: HRESP Not OK");
                w.hno().set_bit();
            }

            if r.rxovr().bit_is_set() {
                defmt::error!("[RSR]: Receive Overrun");
                w.rxovr().set_bit();
            }

            if r.rec().bit_is_set() {
                defmt::info!("[RSR]: Frame Received");
                w.rec().set_bit();
            }

            let bna = r.bna().bit_is_set();
            if bna != self.last_bna {
                self.last_bna = bna;
                defmt::info!("[RSR]: BNA changed to {=bool}", bna);
            }

            w
        });

        // Query Stats
        let timer = GlobalRollingTimer::default();
        if timer.seconds_since(self.last_stat_poll) >= 10 {
            self.last_stat_poll = timer.get_ticks();
            defmt::info!(
                "Frames Transmitted: {=u32}",
                self.periph.gmac_ft.read().bits()
            );
            defmt::info!("Frames Received: {=u32}", self.periph.gmac_fr.read().bits());

            defmt::info!(
                "Transmit Underruns: {=u32}",
                self.periph.gmac_tur.read().bits()
            );
            defmt::info!(
                "Single Collision Frames: {=u32}",
                self.periph.gmac_scf.read().bits()
            );
            defmt::info!(
                "Frames Check Seq Errors: {=u32}",
                self.periph.gmac_fcse.read().bits()
            );
            defmt::info!(
                "Frame Length Field Errors: {=u32}",
                self.periph.gmac_lffe.read().bits()
            );

            defmt::info!(
                "IP Header Checksum Errors: {=u32}",
                self.periph.gmac_ihce.read().bits()
            );
            defmt::info!(
                "TCP Checksum Errors: {=u32}",
                self.periph.gmac_tce.read().bits()
            );
            defmt::info!(
                "UDP Checksum Errors: {=u32}",
                self.periph.gmac_uce.read().bits()
            );
        }
    }

    /// Attempt to read a frame from the hardware receive buffers
    ///
    /// If a frame has been received, a [ReadFrame](ReadFrame) will be returned.
    pub fn read_frame(&mut self) -> Option<ReadFrame> {
        // Scan through the read frames, and attempt to find one marked as "used"
        RX_BUF_DESCS.iter().find_map(|desc| {
            let w0 = desc.get_word_0();
            let addr = w0 & 0xFFFF_FFFC;
            let ready = (w0 & 0x0000_0001) != 0;

            if ready && (addr == 0) {
                defmt::panic!("Packet is ready, but the address is zero.");
            }

            if ready && (addr != 0) {
                // Erase address, but leave 'ready' and potentially 'last' bit set.
                desc.set_word_0(w0 & 0x0000_0003);
                let len = (desc.get_word_1() & 0x0000_0FFF) as usize;

                // Perform a fence to ensure data is correctly flushed before creating a slice.
                fence(Ordering::SeqCst);

                let desc_addr = NonNull::new(desc.words.get().cast())?;
                let buf_addr =
                    NonNull::new(addr as *const [u8; RX_BUF_SIZE] as *mut [u8; RX_BUF_SIZE])?;
                Some(ReadFrame {
                    bufr: buf_addr,
                    len,
                    desc: desc_addr,
                })
            } else {
                None
            }
        })
    }

    /// Attempt to reserve a position in the outgoing frame queue
    ///
    /// If a spot is available, a [WriteFrame](WriteFrame) will be
    /// returned. The frame will NOT be sent over the GMAC interface
    /// until the [send()][WriteFrame::send()] function is called,
    /// consuming the WriteFrame.
    pub fn alloc_write_frame(&mut self) -> Option<WriteFrame> {
        defmt::trace!("TSR: {=u32:08x}", self.periph.gmac_tsr.read().bits());

        let desc = &TX_BUF_DESCS[self.next_tx_idx];
        let w1 = desc.get_word_1();

        // Is this packet ready to be used by software?
        let ready = (w1 & 0x8000_0000) != 0;
        if !ready {
            return None;
        }

        // Yes, it is. Clear out old status registers, but leave the used (and wrap) bit set.
        // Also update the "next index".
        let wrap_bit = w1 & 0x4000_0000;
        desc.set_word_1(0x8000_0000 | wrap_bit);

        let cur_idx = self.next_tx_idx;

        self.next_tx_idx = (cur_idx + 1) % NUM_TX_BUFS;

        Some(WriteFrame {
            bufr: NonNull::new(TX_BUFS[cur_idx].buf.get().cast())?,
            desc: NonNull::new(TX_BUF_DESCS[cur_idx].words.get().cast())?,
            was_sent: false,
        })
    }

    /// Enable the MIIM (PHY) management port.
    ///
    /// This is necessary before calling the other `miim_*` methods.
    fn miim_mgmt_port_enable(&mut self) {
        self.periph.gmac_ncr.modify(|_r, w| w.mpe().set_bit());
    }

    /// Disable the MIIM (PHY) management port.
    fn miim_mgmt_port_disable(&mut self) {
        self.periph.gmac_ncr.modify(|_r, w| w.mpe().clear_bit());
    }

    /// Is the PHY currently busy processing a command/request?
    fn miim_is_busy(&mut self) -> bool {
        self.periph.gmac_nsr.read().idle().bit_is_clear()
    }

    /// Write data to a PHY register
    fn miim_write_data(&mut self, reg_idx: u8, op_data: u16) {
        self.periph.gmac_man.write(|w| {
            w.wzo().clear_bit();
            w.cltto().set_bit();
            unsafe {
                w.op().bits(0b01);
                w.wtn().bits(0b10);
                // TODO: Hardcoded PHY Address
                w.phya().bits(0);
                w.rega().bits(reg_idx);
                w.data().bits(op_data);
            }
            w
        });
    }

    /// Start the async read of data from a PHY register
    fn miim_start_read(&mut self, reg_idx: u8) {
        self.periph.gmac_man.write(|w| {
            w.wzo().clear_bit();
            w.cltto().set_bit();
            unsafe {
                w.op().bits(0b10);
                w.wtn().bits(0b10);
                // TODO: Hardcoded PHY Address
                w.phya().bits(0);
                w.rega().bits(reg_idx);
                w.data().bits(0);
            }
            w
        });
    }

    /// Obtain the data requested by `miim_start_read()`
    fn miim_read_data_get(&mut self) -> u16 {
        self.periph.gmac_man.read().data().bits()
    }

    /// Perform PHY setup procedures.
    fn miim_post_setup(&mut self) {
        let timer = GlobalRollingTimer::default();

        defmt::info!("Starting MIIM setup");
        defmt::info!("Enabling management port...");
        self.miim_mgmt_port_enable();

        defmt::info!("Waiting for miim idle...");
        let val = self.periph.gmac_nsr.read().bits();
        defmt::info!("{=u32:08X}", val);
        while self.miim_is_busy() {}

        defmt::info!("Reset PHY...");

        self.miim_write_data(0, 0b1000_0000_0000_0000); // 0.15: Software reset
        let start = timer.get_ticks();

        // TODO: How long SHOULD this be?
        while self.miim_is_busy() || (timer.millis_since(start) < 5) {}

        self.miim_start_read(0);
        while self.miim_is_busy() {}
        let val = self.miim_read_data_get();
        defmt::info!("New Reg 0 Val: {=u16:04X}", val);

        // TODO: Skipping Autonegotiation Adv step (reg 4)...
        // TODO: Skipping Autonegotiation restart since we didn't change anything...

        // Wait for link to come up
        loop {
            self.miim_start_read(1);
            while self.miim_is_busy() {}
            let val = self.miim_read_data_get();
            // 0b0000_0000_0000_0100
            if (val & 0x0004) != 0 {
                defmt::info!("Link up!");
                break;
            }
        }

        self.miim_mgmt_port_disable();
    }

    // TODO(AJM): Add docs
    fn init(&mut self) {
        // Based on DRV_PIC32CGMAC_LibInit
        // //disable Tx
        // GMAC_REGS->GMAC_NCR &= ~GMAC_NCR_TXEN_Msk;
        // //disable Rx
        // GMAC_REGS->GMAC_NCR &= ~GMAC_NCR_RXEN_Msk;
        self.periph.gmac_ncr.modify(|_r, w| {
            w.txen().clear_bit();
            w.rxen().clear_bit();
            w
        });

        if self.miim_is_busy() {
            defmt::warn!("Busy at start???");
        }

        // //disable all GMAC interrupts for QUEUE 0
        // GMAC_REGS->GMAC_IDR = GMAC_INT_ALL;
        self.periph
            .gmac_idr
            .write(|w| unsafe { w.bits(0xFFFF_FFFF) });

        // //disable all GMAC interrupts for QUEUE 1
        // GMAC_REGS->GMAC_IDRPQ[0] = GMAC_INT_ALL;
        // //disable all GMAC interrupts for QUEUE 2
        // GMAC_REGS->GMAC_IDRPQ[1] = GMAC_INT_ALL;
        // //disable all GMAC interrupts for QUEUE 3
        // GMAC_REGS->GMAC_IDRPQ[2] = GMAC_INT_ALL;
        // //disable all GMAC interrupts for QUEUE 4
        // GMAC_REGS->GMAC_IDRPQ[3] = GMAC_INT_ALL;
        // //disable all GMAC interrupts for QUEUE 5
        // GMAC_REGS->GMAC_IDRPQ[4] = GMAC_INT_ALL;
        for i in 0..5 {
            self.periph.gmac_idrpq[i].write(|w| unsafe { w.bits(0xFFFF_FFFF) });
        }

        // //Clear statistics register
        // GMAC_REGS->GMAC_NCR |=  GMAC_NCR_CLRSTAT_Msk;
        self.periph.gmac_ncr.modify(|_r, w| w.clrstat().set_bit());

        // //Clear RX Status
        // GMAC_REGS->GMAC_RSR =  GMAC_RSR_RXOVR_Msk | GMAC_RSR_REC_Msk | GMAC_RSR_BNA_Msk  | GMAC_RSR_HNO_Msk;
        self.periph.gmac_rsr.write(|w| {
            w.rxovr().set_bit();
            w.rec().set_bit();
            w.bna().set_bit();
            w.hno().set_bit();
            w
        });

        // //Clear TX Status
        // GMAC_REGS->GMAC_TSR = GMAC_TSR_UBR_Msk  | GMAC_TSR_COL_Msk  | GMAC_TSR_RLE_Msk | GMAC_TSR_TXGO_Msk |
        //                                         GMAC_TSR_TFC_Msk  | GMAC_TSR_TXCOMP_Msk  | GMAC_TSR_HRESP_Msk;
        self.periph.gmac_tsr.write(|w| {
            w.ubr().set_bit();
            w.col().set_bit();
            w.rle().set_bit();
            w.txgo().set_bit();
            w.tfc().set_bit();
            w.txcomp().set_bit();
            w.hresp().set_bit();
            w
        });

        // //Clear all GMAC Interrupt status
        // GMAC_REGS->GMAC_ISR;
        let _ = self.periph.gmac_isr.read().bits();

        // GMAC_REGS->GMAC_ISRPQ[0] ;
        // GMAC_REGS->GMAC_ISRPQ[1] ;
        // GMAC_REGS->GMAC_ISRPQ[2] ;
        // GMAC_REGS->GMAC_ISRPQ[3] ;
        // GMAC_REGS->GMAC_ISRPQ[4] ;
        for i in 0..5 {
            let _ = self.periph.gmac_isrpq[i].read().bits();
        }

        // //Set network configurations like speed, full duplex, copy all frames, no broadcast,
        // // pause enable, remove FCS, MDC clock
        // GMAC_REGS->GMAC_NCFGR = GMAC_NCFGR_SPD(1) | GMAC_NCFGR_FD(1) | GMAC_NCFGR_DBW(0) | GMAC_NCFGR_CLK(4)  | GMAC_NCFGR_PEN(1)  | GMAC_NCFGR_RFCS(1);
        // if((pMACDrv->sGmacData.gmacConfig.checksumOffloadRx) != TCPIP_MAC_CHECKSUM_NONE)
        // {
        //     GMAC_REGS->GMAC_NCFGR |= GMAC_NCFGR_RXCOEN_Msk;
        // }
        self.periph.gmac_ncfgr.write(|w| {
            w.spd().set_bit();
            w.fd().set_bit();
            unsafe {
                // 0 = 32-bit data bus
                w.dbw().bits(0);
            }
            // 4 == mck / 64
            w.clk().mck_64();
            w.pen().set_bit();
            w.rfcs().clear_bit();
            // Note: Always enabling checksum offloading for now
            w.rxcoen().set_bit();
            w
        });

        // // Set MAC address
        // DRV_PIC32CGMAC_LibSetMacAddr((const uint8_t *)(pMACDrv->sGmacData.gmacConfig.macAddress.v));
        self.periph.gmac_sa1.gmac_sab.write(|w| unsafe {
            let bottom: u32 = {
                ((self.mac_addr[3] as u32) << 24)
                    | ((self.mac_addr[2] as u32) << 16)
                    | ((self.mac_addr[1] as u32) << 8)
                    | (self.mac_addr[0] as u32)
            };
            w.addr().bits(bottom)
        });
        self.periph.gmac_sa1.gmac_sat.write(|w| unsafe {
            let top: u16 = { ((self.mac_addr[5] as u16) << 8) | (self.mac_addr[4] as u16) };
            w.addr().bits(top)
        });

        // // MII mode config
        // //Configure in RMII mode
        // if((TCPIP_INTMAC_PHY_CONFIG_FLAGS) & DRV_ETHPHY_CFG_RMII)
        //     GMAC_REGS->GMAC_UR = GMAC_UR_RMII(0); //initial mode set as RMII
        // else
        //     GMAC_REGS->GMAC_UR = GMAC_UR_RMII(1); //initial mode set as MII

        // We have an RMII Phy.
        self.periph.gmac_ur.write(|w| {
            // 0 => RMII
            // 1 => MII
            w.rmii().clear_bit()
        });

        // DRV_PIC32CGMAC_LibRxFilterHash_Calculate
        //
        // Note: Set to all 1's to accept all multi-cast addresses
        self.periph
            .gmac_hrb
            .write(|w| unsafe { w.addr().bits(0xFFFF_FFFF) });
        self.periph
            .gmac_hrt
            .write(|w| unsafe { w.addr().bits(0xFFFF_FFFF) });

        // _DRV_GMAC_MacToEthFilter
        //
        // Let's just leave these as defaults, they look sane, but meh.

        // DRV_PIC32CGMAC_LibRxQueFilterInit
        //
        // Let's skip priority filters for now...

        // DRV_PIC32CGMAC_LibRxInit
        //
        // This boils down to a single write to GMAC_RBQB (or GMAC_RBQBAPQ), I think this means
        // I need to set up the receive buffers. NOTE: I think they need to be 8-byte aligned (or something?)
        // (datasheet says 4-byte aligned...)
        //
        // Table 38-2 describes "Receive Buffer Descriptor Entry"
        unsafe {
            // Set the receive buffer addresses in the upper word
            for (desc, buf) in RX_BUF_DESCS.iter().zip(RX_BUFS.iter()) {
                // Take the buffer pointer...
                let buf_addr_ptr: *mut u8 = buf.buf.get().cast();
                let buf_wrd_raw: u32 = buf_addr_ptr as u32;
                let buf_wrd_msk: u32 = buf_wrd_raw & 0xFFFF_FFFC;
                defmt::assert_eq!(buf_wrd_raw, buf_wrd_msk, "RX Buf Alignment Wrong!");

                // ...and store it in the buffer descriptor
                desc.set_word_0(buf_wrd_msk);
            }

            // This is probably UB and should be fixed...
            let last = &RX_BUF_DESCS[NUM_RX_BUFS - 1];
            let mut word_0 = last.get_word_0();

            // Mark as last buffer
            word_0 |= 0x0000_0002;
            last.set_word_0(word_0);
        }

        // TODO: I *think* I need to set DCFGR.DRBS = (1024 / 64) = 16 = 0x10
        // This is done "later" in DRV_PIC32CGMAC_LibInitTransfer

        self.periph.gmac_rbqb.write(|w| unsafe {
            // Take the buffer descriptor pointer...
            let desc_ptr: *const RxBufferDescriptor = RX_BUF_DESCS.as_ptr();
            let desc_wrd_raw: u32 = desc_ptr as u32;
            let desc_wrd_msk: u32 = desc_wrd_raw & 0xFFFF_FFFC;

            defmt::assert_eq!(desc_wrd_raw, desc_wrd_msk, "RX Buf Desc Alignment Wrong!");

            // ... and store it in the RBQB register
            w.bits(desc_wrd_msk)
        });

        // DRV_PIC32CGMAC_LibTxInit
        //
        // Again, this boils down to essentially a single write to GMAC_TBQB, similar to above.
        //
        // Table 38-3 describes "Transmit Buffer Descriptor Entry"
        unsafe {
            // Set the transmit buffer addresses in the upper word
            for (desc, buf) in TX_BUF_DESCS.iter().zip(TX_BUFS.iter()) {
                // Take the buffer pointer...
                let buf_addr_ptr: *mut u8 = buf.buf.get().cast();
                let buf_wrd_raw: u32 = buf_addr_ptr as u32;
                let buf_wrd_msk: u32 = buf_wrd_raw & 0xFFFF_FFFC;
                defmt::assert_eq!(buf_wrd_raw, buf_wrd_msk, "TX Buf Alignment Wrong!");

                // ...and store it in the buffer descriptor
                desc.set_word_0(buf_wrd_msk);

                // Mark this buffer as "used" by software, so the hardware will
                // not attempt to use this buffer until later.
                desc.set_word_1(0x8000_0000);
            }

            // This is probably UB and should be fixed...
            let last = &TX_BUF_DESCS[NUM_TX_BUFS - 1];
            let mut word_1 = last.get_word_1();

            // Mark as wrap buffer
            word_1 |= 0x4000_0000;
            last.set_word_1(word_1);
        }

        self.periph.gmac_tbqb.write(|w| unsafe {
            // Take the buffer descriptor pointer...
            let desc_ptr: *const TxBufferDescriptor = TX_BUF_DESCS.as_ptr();
            let desc_wrd_raw: u32 = desc_ptr as u32;
            let desc_wrd_msk: u32 = desc_wrd_raw & 0xFFFF_FFFC;

            defmt::assert_eq!(desc_wrd_raw, desc_wrd_msk, "TX Buf Desc Alignment Wrong!");

            // ... and store it in the TBQB register
            w.bits(desc_wrd_msk)
        });

        // Note! We need to stub out the prio queues
        for buf in self.periph.gmac_tbqbapq.iter() {
            // Take the buffer descriptor pointer...
            let desc_ptr: *const TxBufferDescriptor = &UNUSED_TX_BUF_DESC;
            let desc_wrd_raw: u32 = desc_ptr as u32;
            let desc_wrd_msk: u32 = desc_wrd_raw & 0xFFFF_FFFC;

            defmt::assert_eq!(desc_wrd_raw, desc_wrd_msk, "TX Buf Desc Alignment Wrong!");

            // ... and store it in the TBQB register
            buf.write(|w| unsafe { w.bits(desc_wrd_msk) });
        }

        // DRV_PIC32CGMAC_LibInitTransfer
        let drbs = (RX_BUF_SIZE / 64).min(255) as u8;
        defmt::assert_ne!(drbs, 0, "Invalid RX Buffer size!");

        self.periph.gmac_dcfgr.write(|w| {
            // ? - DMA Discard Receive Packets
            //
            // 0 - Received packets are stored in the SRAM based packet buffer until next AHB buffer
            // resource becomes available.
            //
            // 1 - Receive packets from the receiver packet buffer memory are automatically discarded when
            // no AHB resource is available.
            //
            // TODO: Example code sets this, so let's do that for now.
            w.ddrp().set_bit();
            unsafe {
                // DRBS is defined in multiples of 64-bytes
                w.drbs().bits(drbs);
            }
            w.txcoen().set_bit(); // Enable Checksum Offload
            w.txpbms().set_bit(); // Use full 4KiB of TX space (???)
            w.rxbms().full(); // Use full 4KiB of RX space (???)
            w.espa().clear_bit(); // Disable endianness swap for packet data access
            w.esma().clear_bit(); // Disable endianness swap for management desc access
            w.fbldo().incr4(); // AHB increments of 4 (???)

            w
        });

        // TODO(AJM): We do NOT enable any interrupts at this point. For early bringup,
        // I plan to poll the relevant status registers. This will change at some point.
        //
        // This note applies to the behavior at the end of DRV_PIC32CGMAC_LibInitTransfer,
        // as well as the next two steps.

        // DRV_PIC32CGMAC_LibSysIntStatus_Clear
        // DRV_PIC32CGMAC_LibSysInt_Enable

        // DRV_PIC32CGMAC_LibTransferEnable
        self.periph.gmac_ncr.modify(|_r, w| {
            w.txen().set_bit();
            w.rxen().set_bit();
            w.westat().set_bit();
            w
        });
    }
}

/// A buffer descriptor for the incoming PHY queue.
#[repr(C, align(8))]
struct RxBufferDescriptor {
    words: UnsafeCell<[u32; 2]>,
}

impl RxBufferDescriptor {
    fn get_word_0(&self) -> u32 {
        unsafe {
            self.words
                .get()
                .cast::<u32>()
                // .add(0)
                .read_volatile()
        }
    }

    fn set_word_0(&self, val: u32) {
        unsafe {
            self.words
                .get()
                .cast::<u32>()
                // .add(0)
                .write_volatile(val)
        }
    }

    fn get_word_1(&self) -> u32 {
        unsafe { self.words.get().cast::<u32>().add(1).read_volatile() }
    }

    // NOTE: the software never really needs to set word 1 of the RxBufferDescriptor,
    // as this contains status codes reported by the hardware.
    //
    // This would change if we did more robust status checking and reporting of the
    // incoming messages - which we don't.
    #[allow(dead_code)]
    fn set_word_1(&self, val: u32) {
        unsafe { self.words.get().cast::<u32>().add(1).write_volatile(val) }
    }
}

/// A buffer descriptor for the outgoing PHY queue
#[repr(C, align(8))]
struct TxBufferDescriptor {
    // TODO: Bitfields for this!
    words: UnsafeCell<[u32; 2]>,
}

impl TxBufferDescriptor {
    // NOTE: the software never really needs to read word 0 of the TxBufferDescriptor,
    // as this contains the pointer of the TX buffer, which the software writes.
    //
    // This would change if we used a more complex TX buffer chain, where we need to
    // actually check the TX buffer address for some reason.
    #[allow(dead_code)]
    fn get_word_0(&self) -> u32 {
        unsafe {
            self.words
                .get()
                .cast::<u32>()
                // .add(0)
                .read_volatile()
        }
    }

    fn set_word_0(&self, val: u32) {
        unsafe {
            self.words
                .get()
                .cast::<u32>()
                // .add(0)
                .write_volatile(val)
        }
    }

    fn get_word_1(&self) -> u32 {
        unsafe { self.words.get().cast::<u32>().add(1).read_volatile() }
    }

    fn set_word_1(&self, val: u32) {
        unsafe { self.words.get().cast::<u32>().add(1).write_volatile(val) }
    }
}

/// An ethernet frame receive buffer. This is primarily used to enforce
/// proper alignment.
#[repr(C, align(8))]
struct RxBuffer {
    buf: UnsafeCell<[u8; RX_BUF_SIZE]>,
}

/// An ethernet frame receive buffer. This is primarily used to enforce
/// proper alignment.
#[repr(C, align(8))]
struct TxBuffer {
    buf: UnsafeCell<[u8; TX_BUF_SIZE]>,
}

unsafe impl Sync for RxBuffer {}
unsafe impl Sync for TxBuffer {}
unsafe impl Sync for RxBufferDescriptor {}
unsafe impl Sync for TxBufferDescriptor {}
