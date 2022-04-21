use core::marker::PhantomData;
use crate::pmc::Pmc;

// PinModes
pub struct Default;
pub struct Output;
pub struct Input;
pub struct PeriphA;
pub struct PeriphB;
pub struct PeriphC;
pub struct PeriphD;

pub enum Level {
    Low,
    High,
}

pub struct Pio<PORT: sealed::Port> {
    periph: PORT,
}

pub struct PortToken<PORT: sealed::Port> {
    pd: PhantomData<PORT>
}

// NOTE: `PIN` must be 0..=31.
pub struct Pin<PORT: sealed::Port, MODE, const PIN: u8> {
    pd: PhantomData<(PORT, MODE)>,
}

impl<PORT: sealed::Port, MODE, const PIN: u8> Pin<PORT, MODE, PIN> {
    #[inline(always)]
    fn new() -> Self {
        Self { pd: PhantomData }
    }

    // TODO: We could also achieve safety here by using a critical sectiom
    pub fn into_periph_mode_a(self, _port_tok: &mut PortToken<PORT>) -> Pin<PORT, PeriphA, PIN> {
        // SAFETY: By having an exclusive ref to the PortToken, we know we have
        // exclusive access to the port configuration registers, avoiding a race on
        // ABCDSR registers.
        //
        // We could also avoid the need for the &mut PortToken if we were to take a critical
        // section instead.
        let port = unsafe { &*PORT::PTR };
        let pinmask = 1 << (PIN as u32);

        // Mode A means [0] = 0, [1] = 0.
        port.pio_abcdsr[0].modify(|r, w| unsafe {
            let initial = r.bits();
            // Mask off the bit
            let setval = initial & !pinmask;
            w.bits(setval)
        });
        port.pio_abcdsr[1].modify(|r, w| unsafe {
            let initial = r.bits();
            // Mask off the bit
            let setval = initial & !pinmask;
            w.bits(setval)
        });

        // Enable peripheral mode
        port.pio_pdr.write(|w| unsafe {
            w.bits(pinmask)
        });

        Pin {
            pd: PhantomData,
        }
    }

    pub fn into_periph_mode_b(self, _port_tok: &mut PortToken<PORT>) -> Pin<PORT, PeriphB, PIN> {
        // SAFETY: By having an exclusive ref to the PortToken, we know we have
        // exclusive access to the port configuration registers, avoiding a race on
        // ABCDSR registers.
        //
        // We could also avoid the need for the &mut PortToken if we were to take a critical
        // section instead.
        let port = unsafe { &*PORT::PTR };
        let pinmask = 1 << (PIN as u32);

        // Mode B means [0] = 1, [1] = 0.
        port.pio_abcdsr[0].modify(|r, w| unsafe {
            let initial = r.bits();
            // Set the bit
            let setval = initial | pinmask;
            w.bits(setval)
        });
        port.pio_abcdsr[1].modify(|r, w| unsafe {
            let initial = r.bits();
            // Mask off the bit
            let setval = initial & !pinmask;
            w.bits(setval)
        });

        // Enable peripheral mode
        port.pio_pdr.write(|w| unsafe {
            w.bits(pinmask)
        });

        Pin {
            pd: PhantomData,
        }
    }

    pub fn into_periph_mode_c(self, _port_tok: &mut PortToken<PORT>) -> Pin<PORT, PeriphC, PIN> {
        // SAFETY: By having an exclusive ref to the PortToken, we know we have
        // exclusive access to the port configuration registers, avoiding a race on
        // ABCDSR registers.
        //
        // We could also avoid the need for the &mut PortToken if we were to take a critical
        // section instead.
        let port = unsafe { &*PORT::PTR };
        let pinmask = 1 << (PIN as u32);

        // Mode C means [0] = 0, [1] = 0.
        port.pio_abcdsr[0].modify(|r, w| unsafe {
            let initial = r.bits();
            // Mask off the bit
            let setval = initial & !pinmask;
            w.bits(setval)
        });
        port.pio_abcdsr[1].modify(|r, w| unsafe {
            let initial = r.bits();
            // Set the bit
            let setval = initial | pinmask;
            w.bits(setval)
        });

        // Enable peripheral mode
        port.pio_pdr.write(|w| unsafe {
            w.bits(pinmask)
        });

        Pin {
            pd: PhantomData,
        }
    }

    pub fn into_periph_mode_d(self, _port_tok: &mut PortToken<PORT>) -> Pin<PORT, PeriphD, PIN> {
        // SAFETY: By having an exclusive ref to the PortToken, we know we have
        // exclusive access to the port configuration registers, avoiding a race on
        // ABCDSR registers.
        //
        // We could also avoid the need for the &mut PortToken if we were to take a critical
        // section instead.
        let port = unsafe { &*PORT::PTR };
        let pinmask = 1 << (PIN as u32);

        // Mode D means [0] = 1, [1] = 1.
        port.pio_abcdsr[0].modify(|r, w| unsafe {
            let initial = r.bits();
            // Mask off the bit
            let setval = initial | pinmask;
            w.bits(setval)
        });
        port.pio_abcdsr[1].modify(|r, w| unsafe {
            let initial = r.bits();
            // Mask off the bit
            let setval = initial | pinmask;
            w.bits(setval)
        });

        // Enable peripheral mode
        port.pio_pdr.write(|w| unsafe {
            w.bits(pinmask)
        });

        Pin {
            pd: PhantomData,
        }
    }
}

pub struct SplitPort<PORT: sealed::Port> {
    pub token: PortToken<PORT>,
    pub p00: Pin<PORT, Default, 00>,
    pub p01: Pin<PORT, Default, 01>,
    pub p02: Pin<PORT, Default, 02>,
    pub p03: Pin<PORT, Default, 03>,
    pub p04: Pin<PORT, Default, 04>,
    pub p05: Pin<PORT, Default, 05>,
    pub p06: Pin<PORT, Default, 06>,
    pub p07: Pin<PORT, Default, 07>,
    pub p08: Pin<PORT, Default, 08>,
    pub p09: Pin<PORT, Default, 09>,
    pub p10: Pin<PORT, Default, 10>,
    pub p11: Pin<PORT, Default, 11>,
    pub p12: Pin<PORT, Default, 12>,
    pub p13: Pin<PORT, Default, 13>,
    pub p14: Pin<PORT, Default, 14>,
    pub p15: Pin<PORT, Default, 15>,
    pub p16: Pin<PORT, Default, 16>,
    pub p17: Pin<PORT, Default, 17>,
    pub p18: Pin<PORT, Default, 18>,
    pub p19: Pin<PORT, Default, 19>,
    pub p20: Pin<PORT, Default, 20>,
    pub p21: Pin<PORT, Default, 21>,
    pub p22: Pin<PORT, Default, 22>,
    pub p23: Pin<PORT, Default, 23>,
    pub p24: Pin<PORT, Default, 24>,
    pub p25: Pin<PORT, Default, 25>,
    pub p26: Pin<PORT, Default, 26>,
    pub p27: Pin<PORT, Default, 27>,
    pub p28: Pin<PORT, Default, 28>,
    pub p29: Pin<PORT, Default, 29>,
    pub p30: Pin<PORT, Default, 30>,
    pub p31: Pin<PORT, Default, 31>,
}

impl<PORT> Pio<PORT>
where
    PORT: sealed::Port
{
    pub fn new(periph: PORT, pmc: &mut Pmc) -> Result<Self, ()> {
        // Should be impossible, but okay.
        pmc.enable_peripherals(&[PORT::PID]).map_err(drop)?;

        Ok(Self {
            periph,
        })
    }

    pub fn split(self) -> SplitPort<PORT> {
        // Configure all pins to an expected state.

        // Set all pins to GPIO/"PIO" mode (not peripheral mode)
        self.periph.pio_per.write(|w| unsafe {
            w.bits(0xFFFF_FFFF)
        });

        // Disable "Multi Drive" mode
        self.periph.pio_mddr.write(|w| unsafe {
            w.bits(0xFFFF_FFFF)
        });

        // Disable Pull Up resistors
        self.periph.pio_pudr.write(|w| unsafe {
            w.bits(0xFFFF_FFFF)
        });

        // Disable Pull Down resistors
        self.periph.pio_ppddr.write(|w| unsafe {
            w.bits(0xFFFF_FFFF)
        });

        // Disable Output Write registers
        self.periph.pio_owdr.write(|w| unsafe {
            w.bits(0xFFFF_FFFF)
        });

        // Disable output mode
        self.periph.pio_odr.write(|w| unsafe {
            w.bits(0xFFFF_FFFF)
        });

        // Set initial output state to low
        self.periph.pio_odsr.write(|w| unsafe {
            w.bits(0x0000_0000)
        });

        // Set initial drive strength to low
        self.periph.pio_driver.write(|w| unsafe {
            w.bits(0x0000_0000)
        });

        SplitPort {
            token: PortToken { pd: PhantomData },
            p00: Pin::new(),
            p01: Pin::new(),
            p02: Pin::new(),
            p03: Pin::new(),
            p04: Pin::new(),
            p05: Pin::new(),
            p06: Pin::new(),
            p07: Pin::new(),
            p08: Pin::new(),
            p09: Pin::new(),
            p10: Pin::new(),
            p11: Pin::new(),
            p12: Pin::new(),
            p13: Pin::new(),
            p14: Pin::new(),
            p15: Pin::new(),
            p16: Pin::new(),
            p17: Pin::new(),
            p18: Pin::new(),
            p19: Pin::new(),
            p20: Pin::new(),
            p21: Pin::new(),
            p22: Pin::new(),
            p23: Pin::new(),
            p24: Pin::new(),
            p25: Pin::new(),
            p26: Pin::new(),
            p27: Pin::new(),
            p28: Pin::new(),
            p29: Pin::new(),
            p30: Pin::new(),
            p31: Pin::new(),
        }
    }
}

mod sealed {
    use core::ops::Deref;
    use atsamx7x_hal::target_device::{pioa, PIOA, PIOB, PIOC, PIOD, PIOE};
    use crate::pmc::PeripheralIdentifier;

    pub trait Port: Deref<Target = pioa::RegisterBlock> {
        const PID: PeripheralIdentifier;
        const PTR: *const pioa::RegisterBlock;
    }

    impl Port for PIOA {
        const PID: PeripheralIdentifier = PeripheralIdentifier::PIOA;
        const PTR: *const pioa::RegisterBlock = PIOA::PTR;
    }
    impl Port for PIOB {
        const PID: PeripheralIdentifier = PeripheralIdentifier::PIOB;
        const PTR: *const pioa::RegisterBlock = PIOB::PTR;
    }
    impl Port for PIOC {
        const PID: PeripheralIdentifier = PeripheralIdentifier::PIOC;
        const PTR: *const pioa::RegisterBlock = PIOC::PTR;
    }
    impl Port for PIOD {
        const PID: PeripheralIdentifier = PeripheralIdentifier::PIOD;
        const PTR: *const pioa::RegisterBlock = PIOD::PTR;
    }
    impl Port for PIOE {
        const PID: PeripheralIdentifier = PeripheralIdentifier::PIOE;
        const PTR: *const pioa::RegisterBlock = PIOE::PTR;
    }
}
