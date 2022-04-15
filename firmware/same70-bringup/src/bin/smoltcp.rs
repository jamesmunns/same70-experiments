#![no_main]
#![no_std]

use core::marker::PhantomData;
use core::ops::Deref;

use cortex_m::singleton;
use groundhog::RollingTimer;
use same70_bringup::{self as _, fixed_setup, hal, gmac::Gmac}; // global logger + panicking-behavior + memory layout
use same70_bringup::GlobalRollingTimer;
use smoltcp::iface::{Neighbor, InterfaceBuilder, SocketStorage, NeighborCache, Routes, Route, Interface};
use smoltcp::phy::{Device, RxToken, TxToken};
use smoltcp::socket::{TcpSocketBuffer, TcpSocket, TcpState, Dhcpv4Event, Dhcpv4Socket};
use smoltcp::time::Instant;
use smoltcp::wire::{IpCidr, IpAddress, EthernetAddress, HardwareAddress, Ipv4Address, Ipv4Cidr};

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Blink!");

    // Obtain PAC-level access
    let board = hal::target_device::Peripherals::take().unwrap();

    // Setup with general purpose settings
    fixed_setup(&board);
    GlobalRollingTimer::init(board.RTT);
    let timer = GlobalRollingTimer::default();

    defmt::println!("Blankin.");

    defmt::println!("Creating GMAC...");
    let mut gmac = unsafe { Gmac::new(board.GMAC) };

    defmt::println!("Initializing...");
    gmac.init();

    defmt::println!("MIIM setup...");
    gmac.miim_post_setup();

    // same70_bringup::exit();

    defmt::println!("Polling...");

    let ip_addrs: &'static mut _ = singleton!(: [IpCidr; 1] = [
        IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 24),
    ]).unwrap();
    let neighbor_cache: &'static mut _ = singleton!(: [Option<(IpAddress, Neighbor)>; 8] = [None; 8]).unwrap();
    let sockets: &'static mut _ = singleton!(: [SocketStorage<'static>; 8] = [SocketStorage::EMPTY; 8]).unwrap();
    let routes_storage: &'static mut _ = singleton!(: [Option<(IpCidr, Route)>; 1] = [None; 1]).unwrap();
    let routes = Routes::new(routes_storage.as_mut_slice());

    let mut iface = InterfaceBuilder::new(gmac, sockets.as_mut_slice())
        .hardware_addr(EthernetAddress::from_bytes(&[0x04, 0x91, 0x62, 0x01, 0x02, 0x03]).into())
        .neighbor_cache(NeighborCache::new(neighbor_cache.as_mut_slice()))
        .routes(routes)
        .ip_addrs(ip_addrs.as_mut_slice())
        .finalize();

    let server_socket = {
        let rx_data: &'static mut [u8] = singleton!(: [u8; 1024] = [0u8; 1024]).unwrap();
        let tx_data: &'static mut [u8] = singleton!(: [u8; 1024] = [0u8; 1024]).unwrap();
        let tcp_rx_buffer = TcpSocketBuffer::new(rx_data);
        let tcp_tx_buffer = TcpSocketBuffer::new(tx_data);
        TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let dhcp_socket = smoltcp::socket::Dhcpv4Socket::new();
    let server_handle = iface.add_socket(server_socket);
    let dhcp_handle = iface.add_socket(dhcp_socket);
    let start = timer.get_ticks();
    let mut did_listen = false;

    let mut buf = [0u8; 1024];

    let mut last_state = smoltcp::socket::TcpState::Closed;

    loop {
        // Log any relevant events
        iface.device_mut().query();

        // TODO: This will roll over after 145 hours!
        match iface.poll(Instant::from_micros(timer.micros_since(start))) {
            Ok(_) => {},
            Err(e) => {
                defmt::println!("Error: {:?}", e);
            },
        }

        let event = iface.get_socket::<Dhcpv4Socket>(dhcp_handle).poll();
        match event {
            None => {}
            Some(Dhcpv4Event::Configured(config)) => {
                defmt::println!("DHCP config acquired!");

                defmt::println!("IP address:      {}", config.address);
                set_ipv4_addr(&mut iface, config.address);

                if let Some(router) = config.router {
                    defmt::println!("Default gateway: {}", router);
                    iface.routes_mut().add_default_ipv4_route(router).unwrap();
                } else {
                    defmt::println!("Default gateway: None");
                    iface.routes_mut().remove_default_ipv4_route();
                }

                for (i, s) in config.dns_servers.iter().enumerate() {
                    if let Some(s) = s {
                        defmt::println!("DNS server {}:    {}", i, s);
                    }
                }
            }
            Some(Dhcpv4Event::Deconfigured) => {
                defmt::println!("DHCP lost config!");
                set_ipv4_addr(&mut iface, Ipv4Cidr::new(Ipv4Address::UNSPECIFIED, 0));
                iface.routes_mut().remove_default_ipv4_route();
            }
        }

        let socket = iface.get_socket::<TcpSocket>(server_handle);

        let state = socket.state();
        if state != last_state {
            defmt::println!("STATE CHANGE: {=?} => {=?}", last_state, state);
            last_state = state;
        }

        if !socket.is_active() && !socket.is_listening() {
            if !did_listen {
                defmt::println!("Listening...");
                socket.listen(4321).unwrap();
                did_listen = true;
            }
        }

        let mut to_send = None;
        if socket.can_recv() {
            socket.recv(|buffer| {
                defmt::println!("RECV!");
                defmt::println!("    len: {=usize}", buffer.len());
                defmt::println!("    dat: {=[u8]}", buffer);
                buf[..buffer.len()].copy_from_slice(buffer);
                to_send = Some(&buf[..buffer.len()]);
                (buffer.len(), ())
            }).unwrap();
        }

        if let Some(tx) = to_send {
            socket.send_slice(tx).unwrap();
        }

        if let TcpState::CloseWait = state {
            socket.close();
            did_listen = false;
        }
        // let rf = match gmac.read_frame() {
        //     Some(f) => {
        //         let fsl: &[u8] = f.deref();
        //         // defmt::println!("Got Frame #{=u32}! Len: {=usize}, Data:", ctr, fsl.len());
        //         // defmt::println!("{=[u8]:02X}", fsl);
        //         ctr = ctr.wrapping_add(1);
        //         f
        //     }
        //     None => continue,
        // };

        // let mut wf = match gmac.alloc_write_frame() {
        //     Some(wf) => wf,
        //     None => {
        //         defmt::println!("Write alloc failed! Skipping response...");
        //         continue;
        //     }
        // };

        // // ...

        // wf.send(rf.len());
    }
}

fn set_ipv4_addr<DeviceT>(iface: &mut Interface<'_, DeviceT>, cidr: Ipv4Cidr)
where
    DeviceT: for<'d> Device<'d>,
{
    iface.update_ip_addrs(|addrs| {
        let dest = addrs.iter_mut().next().unwrap();
        *dest = IpCidr::Ipv4(cidr);
    });
}
