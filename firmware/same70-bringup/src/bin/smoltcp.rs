#![no_main]
#![no_std]

use cortex_m::singleton;
use groundhog::RollingTimer;
use same70_bringup::{
    efc::Efc,
    gmac::{Gmac, GmacPins},
    hal::target_device::Peripherals,
    pio::Pio,
    pmc::{
        ClockSettings, MainClockOscillatorSource, MasterClockSource, MckDivider, MckPrescaler,
        PeripheralIdentifier, Pmc,
    },
    wdt::Wdt,
    GlobalRollingTimer,
}; // global logger + panicking-behavior + memory layout

use smoltcp::{
    iface::{Interface, InterfaceBuilder, Neighbor, NeighborCache, Route, Routes, SocketStorage},
    phy::Device,
    socket::{Dhcpv4Event, Dhcpv4Socket, TcpSocket, TcpSocketBuffer, TcpState},
    time::Instant,
    wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address, Ipv4Cidr},
};

#[cortex_m_rt::entry]
fn main() -> ! {
    // Obtain PAC-level access
    let board = Peripherals::take().unwrap();

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

    let gmac = defmt::unwrap!(Gmac::new(
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

    let ip_addrs: &'static mut _ = singleton!(: [IpCidr; 1] = [
        IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 24),
    ]).unwrap();
    let neighbor_cache: &'static mut _ = singleton!(: [Option<(IpAddress, Neighbor)>; 8] = [None; 8]).unwrap();
    let sockets: &'static mut _ = singleton!(: [SocketStorage<'static>; 8] = [SocketStorage::EMPTY; 8]).unwrap();
    let routes_storage: &'static mut _ = singleton!(: [Option<(IpCidr, Route)>; 1] = [None; 1]).unwrap();
    let routes = Routes::new(routes_storage.as_mut_slice());

    let mac_addr = gmac.mac_addr();

    let mut iface = InterfaceBuilder::new(gmac, sockets.as_mut_slice())
        .hardware_addr(EthernetAddress::from_bytes(&mac_addr).into())
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

    let timer = GlobalRollingTimer::default();

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
