use atsamx7x_hal::target_device::GMAC;


pub struct Gmac {
    periph: GMAC,
}

impl Gmac {
    // TODO: Mark safe when possible.
    pub unsafe fn new(periph: GMAC) -> Self {
        todo!("Allocate pins, clocks, etc.");

        Self {
            periph
        }
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

// Main chunk of DRV_GMAC_Initialize
// while(1)
// {
//     uint32_t rxfilter= 0;

//     // start the initialization sequence
//     DRV_PIC32CGMAC_LibSysInt_Disable(pMACDrv, GMAC_ALL_QUE_MASK, NULL);

//     initRes = _DRV_GMAC_PHYInitialise(pMACDrv);
//     if(initRes != TCPIP_MAC_RES_OK)
//     {
//         // some error occurred
//         initRes = TCPIP_MAC_RES_PHY_INIT_FAIL;
//         break;
//     }

//     //global configurations for gmac
//     DRV_PIC32CGMAC_LibInit(pMACDrv);

//     //Receive All Multi-cast packets? then set 64-bit hash value to all ones.
//     if((TCPIP_GMAC_RX_FILTERS) & TCPIP_MAC_RX_FILTER_TYPE_MCAST_ACCEPT)
//     {
//         DRV_GMAC_HASH hash;

//         hash.hash_value = -1; //Set 64-bit Hash value to all 1s, to receive all multi-cast
//         hash.calculate_hash = false; // No hash calculation; directly set hash register

//         DRV_PIC32CGMAC_LibRxFilterHash_Calculate(pMACDrv, &hash);
//     }
//     // Set Rx Filters
//     gmacRxFilt = _DRV_GMAC_MacToEthFilter(TCPIP_GMAC_RX_FILTERS);
//     rxfilter = (uint32_t)(GMAC_REGS->GMAC_NCFGR) & (~GMAC_FILT_ALL_FILTERS);
//     GMAC_REGS->GMAC_NCFGR  = (rxfilter|gmacRxFilt) ;

//     // Initialize Rx Queue Filters
//     if(DRV_PIC32CGMAC_LibRxQueFilterInit(pMACDrv) != DRV_PIC32CGMAC_RES_OK)
//     {
//         initRes = TCPIP_MAC_RES_INIT_FAIL;
//         break;
//     }

//     if(DRV_PIC32CGMAC_LibRxInit(pMACDrv) != DRV_PIC32CGMAC_RES_OK)
//     {
//         initRes = TCPIP_MAC_RES_INIT_FAIL;
//         break;
//     }

//     if(DRV_PIC32CGMAC_LibTxInit(pMACDrv) != DRV_PIC32CGMAC_RES_OK)
//     {
//         initRes = TCPIP_MAC_RES_INIT_FAIL;
//         break;
//     }

//     for(queueIdx = GMAC_QUE_0; queueIdx < DRV_GMAC_NUMBER_OF_QUEUES; queueIdx++)
//     {
//         //Initialize QUEUES
//         if(DRV_PIC32CGMAC_LibInitTransfer(pMACDrv,queueIdx) != DRV_PIC32CGMAC_RES_OK)
//         {
//             initRes = TCPIP_MAC_RES_INIT_FAIL;
//             break;
//         }
//     }

//     DRV_PIC32CGMAC_LibSysIntStatus_Clear(pMACDrv, GMAC_ALL_QUE_MASK);
//     DRV_PIC32CGMAC_LibSysInt_Enable(pMACDrv, GMAC_ALL_QUE_MASK);


//     DRV_PIC32CGMAC_LibTransferEnable(pMACDrv); //enable Transmit and Receive of GMAC

//     if(DRV_GMAC_EventInit((DRV_HANDLE)pMACDrv, macControl->eventF, macControl->eventParam) != TCPIP_MAC_RES_OK)
//     {
//         initRes = TCPIP_MAC_RES_EVENT_INIT_FAIL;
//         break;
//     }
//     // end of initialization
//     break;

// }


// void DRV_PIC32CGMAC_LibInit(DRV_GMAC_DRIVER* pMACDrv)
// {

//     //disable Tx
//     GMAC_REGS->GMAC_NCR &= ~GMAC_NCR_TXEN_Msk;
//     //disable Rx
//     GMAC_REGS->GMAC_NCR &= ~GMAC_NCR_RXEN_Msk;

//     //disable all GMAC interrupts for QUEUE 0
//     GMAC_REGS->GMAC_IDR = GMAC_INT_ALL;
//     //disable all GMAC interrupts for QUEUE 1
//     GMAC_REGS->GMAC_IDRPQ[0] = GMAC_INT_ALL;
//     //disable all GMAC interrupts for QUEUE 2
//     GMAC_REGS->GMAC_IDRPQ[1] = GMAC_INT_ALL;
//     //disable all GMAC interrupts for QUEUE 3
//     GMAC_REGS->GMAC_IDRPQ[2] = GMAC_INT_ALL;
//     //disable all GMAC interrupts for QUEUE 4
//     GMAC_REGS->GMAC_IDRPQ[3] = GMAC_INT_ALL;
//     //disable all GMAC interrupts for QUEUE 5
//     GMAC_REGS->GMAC_IDRPQ[4] = GMAC_INT_ALL;

//     //Clear statistics register
//     GMAC_REGS->GMAC_NCR |=  GMAC_NCR_CLRSTAT_Msk;
//     //Clear RX Status
//     GMAC_REGS->GMAC_RSR =  GMAC_RSR_RXOVR_Msk | GMAC_RSR_REC_Msk | GMAC_RSR_BNA_Msk  | GMAC_RSR_HNO_Msk;
//     //Clear TX Status
//     GMAC_REGS->GMAC_TSR = GMAC_TSR_UBR_Msk  | GMAC_TSR_COL_Msk  | GMAC_TSR_RLE_Msk | GMAC_TSR_TXGO_Msk |
//                                             GMAC_TSR_TFC_Msk  | GMAC_TSR_TXCOMP_Msk  | GMAC_TSR_HRESP_Msk;

//     //Clear all GMAC Interrupt status
//     GMAC_REGS->GMAC_ISR;
//     GMAC_REGS->GMAC_ISRPQ[0] ;
//     GMAC_REGS->GMAC_ISRPQ[1] ;
//     GMAC_REGS->GMAC_ISRPQ[2] ;
//     GMAC_REGS->GMAC_ISRPQ[3] ;
//     GMAC_REGS->GMAC_ISRPQ[4] ;
//     //Set network configurations like speed, full duplex, copy all frames, no broadcast,
//     // pause enable, remove FCS, MDC clock
//     GMAC_REGS->GMAC_NCFGR = GMAC_NCFGR_SPD(1) | GMAC_NCFGR_FD(1) | GMAC_NCFGR_DBW(0) | GMAC_NCFGR_CLK(4)  | GMAC_NCFGR_PEN(1)  | GMAC_NCFGR_RFCS(1);

//     if((pMACDrv->sGmacData.gmacConfig.checksumOffloadRx) != TCPIP_MAC_CHECKSUM_NONE)
//     {
//         GMAC_REGS->GMAC_NCFGR |= GMAC_NCFGR_RXCOEN_Msk;
//     }
//     // Set MAC address
//     DRV_PIC32CGMAC_LibSetMacAddr((const uint8_t *)(pMACDrv->sGmacData.gmacConfig.macAddress.v));
//     // MII mode config
//     //Configure in RMII mode
//     if((TCPIP_INTMAC_PHY_CONFIG_FLAGS) & DRV_ETHPHY_CFG_RMII)
//         GMAC_REGS->GMAC_UR = GMAC_UR_RMII(0); //initial mode set as RMII
//     else
//         GMAC_REGS->GMAC_UR = GMAC_UR_RMII(1); //initial mode set as MII
// }

// DRV_PIC32CGMAC_RESULT DRV_PIC32CGMAC_LibSetMacAddr (const uint8_t * pMacAddr)
// {
//     GMAC_REGS->GMAC_SA[0].GMAC_SAB = (pMacAddr[3] << 24)
//                                 | (pMacAddr[2] << 16)
//                                 | (pMacAddr[1] <<  8)
//                                 | (pMacAddr[0]);

//     GMAC_REGS->GMAC_SA[0].GMAC_SAT = (pMacAddr[5] <<  8)
//                                 | (pMacAddr[4]) ;

//     return DRV_PIC32CGMAC_RES_OK;
// }


