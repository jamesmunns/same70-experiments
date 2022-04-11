#![no_main]
#![no_std]

use core::marker::PhantomData;
use core::ops::Deref;

use cortex_m::singleton;
use same70_bringup::{self as _, fixed_setup, hal, gmac::Gmac}; // global logger + panicking-behavior + memory layout
use same70_bringup::GlobalRollingTimer;
use smoltcp::iface::{Neighbor, InterfaceBuilder, SocketStorage, NeighborCache};
use smoltcp::phy::{Device, RxToken, TxToken};
use smoltcp::wire::{IpCidr, IpAddress, EthernetAddress, HardwareAddress};

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

    struct LolDevice<'a> {
        _plt: PhantomData<&'a ()>,
    }
    struct LolRxToken<'a> {
        _plt: PhantomData<&'a ()>,
    }
    struct LolTxToken<'a> {
        _plt: PhantomData<&'a ()>,
    }

    impl<'a> RxToken for LolRxToken<'a> {
        fn consume<R, F>(self, timestamp: smoltcp::time::Instant, f: F) -> smoltcp::Result<R>
        where
            F: FnOnce(&mut [u8]) -> smoltcp::Result<R> {
            todo!()
        }
    }

    impl<'a> TxToken for LolTxToken<'a> {
        fn consume<R, F>(self, timestamp: smoltcp::time::Instant, len: usize, f: F) -> smoltcp::Result<R>
        where
            F: FnOnce(&mut [u8]) -> smoltcp::Result<R> {
            todo!()
        }
    }

    impl<'a, 'b> Device<'a> for LolDevice<'b> {
        type RxToken = LolRxToken<'a>;
        type TxToken = LolTxToken<'a>;

        fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
            todo!()
        }

        fn transmit(&'a mut self) -> Option<Self::TxToken> {
            todo!()
        }

        fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
            todo!()
        }
    }

    let ip_addrs: &'static mut _ = singleton!(: [IpCidr; 1] = [IpCidr::new(IpAddress::v4(192, 168, 240, 0), 24)]).unwrap();
    let neighbor_cache: &'static mut _ = singleton!(: [Option<(IpAddress, Neighbor)>; 8] = [None; 8]).unwrap();
    let sockets: &'static mut _ = singleton!(: [SocketStorage<'static>; 8] = [SocketStorage::EMPTY; 8]).unwrap();

    let iface = InterfaceBuilder::new(LolDevice { _plt: PhantomData }, sockets.as_mut_slice())
        .hardware_addr(EthernetAddress::from_bytes(&[0x04, 0x91, 0x62, 0x01, 0x02, 0x03]).into())
        .neighbor_cache(NeighborCache::new(neighbor_cache.as_mut_slice()))
        .ip_addrs(ip_addrs.as_mut_slice())
        .finalize();

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

        let mut wf = match gmac.alloc_write_frame() {
            Some(wf) => wf,
            None => {
                defmt::println!("Write alloc failed! Skipping response...");
                continue;
            }
        };

        // ...

        wf.send(rf.len());
    }
}
