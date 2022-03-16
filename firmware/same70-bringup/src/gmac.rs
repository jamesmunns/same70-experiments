use atsamx7x_hal::target_device::GMAC;
use groundhog::RollingTimer;

use crate::GlobalRollingTimer;

const NUM_RX_BUFS: usize = 4;
const NUM_TX_BUFS: usize = 4;
const RX_BUF_SIZE: usize = 1024;
const TX_BUF_SIZE: usize = 1024;

// TODO: This needs a specific linker section (probably)
// Todo: UnsafeCell?
static mut RX_BUF_DESCS: [RxBufferDescriptor; NUM_RX_BUFS] = [RX_BUF_DESC_DEFAULT; NUM_RX_BUFS];
static mut RX_BUFS: [RxBuffer; NUM_RX_BUFS] = [RX_BUF_DEFAULT; NUM_RX_BUFS];
static mut TX_BUF_DESCS: [TxBufferDescriptor; NUM_TX_BUFS] = [TX_BUF_DESC_DEFAULT; NUM_TX_BUFS];
static mut TX_BUFS: [TxBuffer; NUM_TX_BUFS] = [TX_BUF_DEFAULT; NUM_TX_BUFS];

pub struct Gmac {
    periph: GMAC,
}

impl Gmac {
    // TODO: Mark safe when possible.
    pub unsafe fn new(periph: GMAC) -> Self {
        defmt::println!("WARNING: Don't forget! We rely on configuration elsewhere for pins and stuff.");

        Self {
            periph
        }
    }

    pub fn did_it_work(&mut self) -> bool {
        unsafe {
            let rx_desc_ptr: *const RxBufferDescriptor = RX_BUF_DESCS.as_ptr();
            let desc = rx_desc_ptr.read_volatile();
            (desc.word_0 & 0x0000_0001) != 0
        }
    }

    // pub fn mac_preinit(&mut self) {
        // DRV_MIIM_Setup:
        //     _DRV_MIIM_ETH_ENABLE
        //         noop
        //     _DRV_MIIM_MII_RELEASE_RESET
        //         noop
        //     _DRV_MIIM_SMIClockSet
        //         This is already handled in `init` by `w.clk().mck_64();` in NCFGR
        //     _DRV_MIIM_SETUP_PREAMBLE
        //         noop
        //     _DRV_MIIM_SCAN_INCREMENT
        //         noop
    // }

    fn miim_mgmt_port_enable(&mut self) {
        self.periph.gmac_ncr.modify(|_r, w| {
            w.mpe().set_bit()
        });
    }

    fn miim_mgmt_port_disable(&mut self) {
        self.periph.gmac_ncr.modify(|_r, w| {
            w.mpe().clear_bit()
        });
    }

    fn miim_is_busy(&mut self) -> bool {
        self.periph.gmac_nsr.read().idle().bit_is_clear()
    }

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

    fn miim_read_data_get(&mut self) -> u16 {
        self.periph.gmac_man.read().data().bits()
    }

    pub fn miim_post_setup(&mut self) {
        let timer = GlobalRollingTimer::default();

        defmt::println!("Starting MIIM setup");
        defmt::println!("Enabling management port...");
        self.miim_mgmt_port_enable();

        defmt::println!("Waiting for miim idle...");
        let val = self.periph.gmac_nsr.read().bits();
        defmt::println!("{=u32:08X}", val);
        while self.miim_is_busy() { }

        defmt::println!("Reset PHY...");

        self.miim_write_data(0, 0x8000); // 0.15: Software reset
        let start = timer.get_ticks();

        // TODO: How long SHOULD this be?
        while self.miim_is_busy() || (timer.millis_since(start) < 5) { }

        self.miim_start_read(0);
        while self.miim_is_busy() { }
        let val = self.miim_read_data_get();
        defmt::println!("New Reg 0 Val: {=u16:04X}", val);

        // TODO: Skipping Autonegotiation Adv step (reg 4)...
        // TODO: Skipping Autonegotiation restart since we didn't change anything...

        // Wait for link to come up
        loop {
            self.miim_start_read(1);
            while self.miim_is_busy() { }
            let val = self.miim_read_data_get();
            if (val & 0x0004) != 0 {
                defmt::println!("Link up!");
                break;
            }
        }
    }

    pub unsafe fn danger_read(&mut self) {
        let rx_desc_ptr: *const RxBufferDescriptor = RX_BUF_DESCS.as_ptr();
        let desc = rx_desc_ptr.read_volatile();
        defmt::println!("RXBD0: {=u32:08X}", desc.word_0);
        defmt::println!("RXBD1: {=u32:08X}", desc.word_1);
        let len = (desc.word_1 & 0x0000_0FFF) as usize;
        defmt::println!("Len: {=usize}", len);

        let rx_buf_ptr: *const RxBuffer = RX_BUFS.as_ptr();
        let buf_cpy = rx_buf_ptr.read_volatile();
        let bufsl = &buf_cpy.buf[..len];
        defmt::println!("Data:");
        defmt::println!("{=[u8]:02X}", bufsl);
    }

    pub fn init(&mut self) {
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
            defmt::println!("Busy at start???");
        }

        // //disable all GMAC interrupts for QUEUE 0
        // GMAC_REGS->GMAC_IDR = GMAC_INT_ALL;
        self.periph.gmac_idr.write(|w| unsafe { w.bits(0xFFFF_FFFF) });

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
        self.periph.gmac_ncr.modify(|_r, w| {
            w.clrstat().set_bit()
        });

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
            w.rfcs().set_bit();
            // Note: Always enabling checksum offloading for now
            w.rxcoen().set_bit();
            w
        });


        // // Set MAC address
        // DRV_PIC32CGMAC_LibSetMacAddr((const uint8_t *)(pMACDrv->sGmacData.gmacConfig.macAddress.v));

        // For now, use (one of) Microchip's MACs. This is temporary.
        //
        // 04-91-62   (hex)        Microchip Technology Inc.
        // 049162     (base 16)    Microchip Technology Inc.
        //                         2355 W. Chandler Blvd.
        //                         Chandler  AZ  85224
        //                         US

        self.periph.gmac_sa1.gmac_sab.write(|w| unsafe {
            w.addr().bits(u32::from_le_bytes([
                0x03, // LSB
                0x02, //
                0x01, //
                0x62, // MSB
            ]))
        });
        self.periph.gmac_sa1.gmac_sat.write(|w| unsafe {
            w.addr().bits(u16::from_le_bytes([
                0x91, // LSB
                0x04, // MSB
            ]))
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
        self.periph.gmac_hrb.write(|w| unsafe { w.addr().bits(0xFFFF_FFFF) });
        self.periph.gmac_hrt.write(|w| unsafe { w.addr().bits(0xFFFF_FFFF) });

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
            for (desc, buf) in RX_BUF_DESCS.iter_mut().zip(RX_BUFS.iter_mut()) {
                // Take the buffer pointer...
                let buf_addr_ptr: *mut u8 = buf.buf.as_mut_ptr();
                let buf_wrd_raw: u32 = buf_addr_ptr as u32;
                let buf_wrd_msk: u32 = buf_wrd_raw & 0xFFFF_FFFC;
                defmt::assert_eq!(buf_wrd_raw, buf_wrd_msk, "RX Buf Alignment Wrong!");

                // ...and store it in the buffer descriptor
                let mut new_desc = RX_BUF_DESC_DEFAULT;
                new_desc.word_0 = buf_wrd_msk;
                let desc_ptr: *mut RxBufferDescriptor = desc;
                desc_ptr.write_volatile(new_desc);
            }

            // This is probably UB and should be fixed...
            let last: *mut _ = &mut RX_BUF_DESCS[NUM_RX_BUFS - 1];
            let mut val = last.read_volatile();

            // Mark as last buffer
            val.word_0 |= 0x0000_0002;
            last.write_volatile(val);
        }

        // TODO: I *think* I need to set DCFGR.DRBS = (1024 / 64) = 16 = 0x10
        // This is done "later" in DRV_PIC32CGMAC_LibInitTransfer

        self.periph.gmac_rbqb.write(|w| unsafe {
            // Take the buffer descriptor pointer...
            let desc_ptr: *mut RxBufferDescriptor = RX_BUF_DESCS.as_mut_ptr();
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
            for (desc, buf) in TX_BUF_DESCS.iter_mut().zip(TX_BUFS.iter_mut()) {
                // Take the buffer pointer...
                let buf_addr_ptr: *mut u8 = buf.buf.as_mut_ptr();
                let buf_wrd_raw: u32 = buf_addr_ptr as u32;
                let buf_wrd_msk: u32 = buf_wrd_raw & 0xFFFF_FFFC;
                defmt::assert_eq!(buf_wrd_raw, buf_wrd_msk, "TX Buf Alignment Wrong!");

                // ...and store it in the buffer descriptor
                let mut new_desc = TX_BUF_DESC_DEFAULT;
                new_desc.word_0 = buf_wrd_msk;

                // Mark this buffer as "used" by software, so the hardware will
                // not attempt to use this buffer until later.
                new_desc.word_1 = 0x8000_0000;
                let desc_ptr: *mut TxBufferDescriptor = desc;
                desc_ptr.write_volatile(new_desc);
            }
            // Note: No need to mark "last buffer" yet.
        }

        self.periph.gmac_tbqb.write(|w| unsafe {
            // Take the buffer descriptor pointer...
            let desc_ptr: *mut TxBufferDescriptor = TX_BUF_DESCS.as_mut_ptr();
            let desc_wrd_raw: u32 = desc_ptr as u32;
            let desc_wrd_msk: u32 = desc_wrd_raw & 0xFFFF_FFFC;

            defmt::assert_eq!(desc_wrd_raw, desc_wrd_msk, "TX Buf Desc Alignment Wrong!");

            // ... and store it in the TBQB register
            w.bits(desc_wrd_msk)
        });

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
            w.txcoen().set_bit();   // Enable Checksum Offload
            w.txpbms().set_bit();   // Use full 4KiB of TX space (???)
            w.rxbms().full();       // Use full 4KiB of RX space (???)
            w.espa().clear_bit();   // Disable endianness swap for packet data access
            w.esma().clear_bit();   // Disable endianness swap for management desc access
            w.fbldo().incr4();      // AHB increments of 4 (???)

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

// Note: MIIM == MDIO == SMI

// Relevant driver call chain
//
// DRV_GMAC_Initialize
//     DRV_PIC32CGMAC_LibSysInt_Disable
//         * Not much, just disabling interrupts?
//     _DRV_GMAC_PHYInitialise
//         * DRV_ETHPHY_Initialize
//             * Data structure init?
//         * DRV_ETHPHY_Open
                // _DRV_ETHPHY_ClientObjectAllocate
                //     * Data structures...
                // DRV_MIIM_Open
                //     _DRV_MIIM_GetObjectAndLock
                //         * Data structures...
                //     _DRV_MIIM_ClientAllocate
                //         * Data structures...
                //     _DRV_MIIM_ObjUnlock
                //         * FreeRTOS stuff?
//     DRV_PIC32CGMAC_LibInit
//         Important! See below
//     DRV_PIC32CGMAC_LibRxFilterHash_Calculate
//         Important! (I think?)
//     _DRV_GMAC_MacToEthFilter
//         Used to calculate GMAC_NCFGR ?
//     DRV_PIC32CGMAC_LibRxQueFilterInit
//         Used to calculate priority filters? Unsure if necessary
//     DRV_PIC32CGMAC_LibRxInit
//     DRV_PIC32CGMAC_LibTxInit
//     for each queue:
//         DRV_PIC32CGMAC_LibInitTransfer
//     DRV_PIC32CGMAC_LibSysIntStatus_Clear
//     DRV_PIC32CGMAC_LibSysInt_Enable
//     DRV_PIC32CGMAC_LibTransferEnable
//     DRV_GMAC_EventInit
//     if failed:
//         _MACDeinit
//     "remaining initialization is done by DRV_ETHMAC_PIC32MACTasks"

#[repr(C, align(8))]
struct RxBufferDescriptor {
    // TODO: Bitfields for this!
    word_0: u32,
    word_1: u32,
}

#[repr(C, align(8))]
struct TxBufferDescriptor {
    // TODO: Bitfields for this!
    word_0: u32,
    word_1: u32,
}

#[repr(C, align(8))]
struct RxBuffer {
    buf: [u8; RX_BUF_SIZE],
}

#[repr(C, align(8))]
struct TxBuffer {
    buf: [u8; TX_BUF_SIZE],
}

const RX_BUF_DEFAULT: RxBuffer = RxBuffer { buf: [0u8; RX_BUF_SIZE] };
const TX_BUF_DEFAULT: TxBuffer = TxBuffer { buf: [0u8; RX_BUF_SIZE] };

const RX_BUF_DESC_DEFAULT: RxBufferDescriptor = RxBufferDescriptor {
    word_0: 0x0000_0000,
    word_1: 0x0000_0000,
};

const TX_BUF_DESC_DEFAULT: TxBufferDescriptor = TxBufferDescriptor {
    word_0: 0x0000_0000,
    word_1: 0x0000_0000,
};
