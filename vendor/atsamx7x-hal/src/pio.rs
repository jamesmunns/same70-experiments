//! PIO - GPIO configuration and functionality
//!
//! At the moment, this module has limited support to allow for basic
//! ethernet, SPI, and GPIO output capabilities.

use crate::pmc::Pmc;
use core::marker::PhantomData;

/// A marker struct representing "GPIO" (rather than "Peripheral") mode.
pub struct Gpio<T> {
    _mode: T,
}

/// A marker struct representing a default/unconfigured mode.
pub struct Unconfigured;

/// A marker struct representing a GPIO in the Output mode.
pub struct Output;

/// A marker struct representing a pin the the Peripheral A mode.
pub struct PeriphA;
/// A marker struct representing a pin the the Peripheral B mode.
pub struct PeriphB;
/// A marker struct representing a pin the the Peripheral C mode.
pub struct PeriphC;
/// A marker struct representing a pin the the Peripheral D mode.
pub struct PeriphD;

/// The output level of a GPIO.
///
/// Low corresponds to GND, High represents VCC
pub enum Level {
    Low,
    High,
}

/// A PIO port, representing ownership of all 32 pins of the port
pub struct Pio<PORT: sealed::Port> {
    periph: PORT,
}

/// A token representing access to the port configuration registers.
///
/// This is used at runtime to ensure pin configuration only occurs
/// in a mutually exclusive ("change one pin at a time") manner to
/// avoid data races.
pub struct PortToken<PORT: sealed::Port> {
    pd: PhantomData<PORT>,
}

/// A single GPIO Pin
// NOTE: `PIN` must be 0..=31.
pub struct Pin<PORT: sealed::Port, MODE, const PIN: u8> {
    pd: PhantomData<(PORT, MODE)>,
}

impl<PORT: sealed::Port, const PIN: u8> Pin<PORT, Gpio<Output>, PIN> {
    /// Set the GPIO pin to the low output state
    pub fn set_low(&mut self) {
        // SAFETY: We only use atomic enable/disable operations here, and only on our one given pin.
        let port = unsafe { &*PORT::PTR };
        let pinmask = 1 << (PIN as u32);
        port.pio_codr.write(|w| unsafe { w.bits(pinmask) });
    }

    /// Set the GPIO pin to the high output state
    pub fn set_high(&mut self) {
        // SAFETY: We only use atomic enable/disable operations here, and only on our one given pin.
        let port = unsafe { &*PORT::PTR };
        let pinmask = 1 << (PIN as u32);
        port.pio_sodr.write(|w| unsafe { w.bits(pinmask) });
    }
}

impl<PORT: sealed::Port, SUBMODE, const PIN: u8> Pin<PORT, Gpio<SUBMODE>, PIN> {
    /// Convert the pin into a push pull output mode
    pub fn into_push_pull_output(self, initial_level: Level) -> Pin<PORT, Gpio<Output>, PIN> {
        // SAFETY: We only use atomic enable/disable operations here, and only on our one given pin.
        let port = unsafe { &*PORT::PTR };
        let pinmask = 1 << (PIN as u32);

        // No need to touch pio_per, as we know the pin is already in GPIO (not Periph) mode

        // Disable "Multi Drive" mode
        port.pio_mddr.write(|w| unsafe { w.bits(pinmask) });

        // Disable Pull Up resistors
        port.pio_pudr.write(|w| unsafe { w.bits(pinmask) });

        // Disable Pull Down resistors
        port.pio_ppddr.write(|w| unsafe { w.bits(pinmask) });

        // Enable Output Write registers
        port.pio_ower.write(|w| unsafe { w.bits(pinmask) });

        // Set initial output state as requested
        // DON'T use ODSR, as that is not an atomic register!
        match initial_level {
            Level::Low => port.pio_codr.write(|w| unsafe { w.bits(pinmask) }),
            Level::High => port.pio_sodr.write(|w| unsafe { w.bits(pinmask) }),
        }

        // TODO: We DON'T set the driver through PIO_DRIVER, as it is not
        // atomic, and would require us to borrow the token back.
        // For now, we never change that at all, so just leave it as-is

        // Enable output mode
        port.pio_oer.write(|w| unsafe { w.bits(pinmask) });

        Pin { pd: PhantomData }
    }
}

impl<PORT: sealed::Port, MODE, const PIN: u8> Pin<PORT, MODE, PIN> {
    #[inline(always)]
    fn new() -> Self {
        Self { pd: PhantomData }
    }

    /// Convert the pin into Peripheral Mode A.
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
        port.pio_pdr.write(|w| unsafe { w.bits(pinmask) });

        Pin { pd: PhantomData }
    }

    /// Convert the pin into Peripheral Mode B.
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
        port.pio_pdr.write(|w| unsafe { w.bits(pinmask) });

        Pin { pd: PhantomData }
    }

    /// Convert the pin into Peripheral Mode C.
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
        port.pio_pdr.write(|w| unsafe { w.bits(pinmask) });

        Pin { pd: PhantomData }
    }

    /// Convert the pin into Peripheral Mode D.
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
        port.pio_pdr.write(|w| unsafe { w.bits(pinmask) });

        Pin { pd: PhantomData }
    }
}

/// A structure containing all 32 pins of the port, as well as a token
/// that can be used for reconfiguration of each pin at a later time.
pub struct SplitPort<PORT: sealed::Port> {
    pub token: PortToken<PORT>,
    pub p00: Pin<PORT, Gpio<Unconfigured>, 00>,
    pub p01: Pin<PORT, Gpio<Unconfigured>, 01>,
    pub p02: Pin<PORT, Gpio<Unconfigured>, 02>,
    pub p03: Pin<PORT, Gpio<Unconfigured>, 03>,
    pub p04: Pin<PORT, Gpio<Unconfigured>, 04>,
    pub p05: Pin<PORT, Gpio<Unconfigured>, 05>,
    pub p06: Pin<PORT, Gpio<Unconfigured>, 06>,
    pub p07: Pin<PORT, Gpio<Unconfigured>, 07>,
    pub p08: Pin<PORT, Gpio<Unconfigured>, 08>,
    pub p09: Pin<PORT, Gpio<Unconfigured>, 09>,
    pub p10: Pin<PORT, Gpio<Unconfigured>, 10>,
    pub p11: Pin<PORT, Gpio<Unconfigured>, 11>,
    pub p12: Pin<PORT, Gpio<Unconfigured>, 12>,
    pub p13: Pin<PORT, Gpio<Unconfigured>, 13>,
    pub p14: Pin<PORT, Gpio<Unconfigured>, 14>,
    pub p15: Pin<PORT, Gpio<Unconfigured>, 15>,
    pub p16: Pin<PORT, Gpio<Unconfigured>, 16>,
    pub p17: Pin<PORT, Gpio<Unconfigured>, 17>,
    pub p18: Pin<PORT, Gpio<Unconfigured>, 18>,
    pub p19: Pin<PORT, Gpio<Unconfigured>, 19>,
    pub p20: Pin<PORT, Gpio<Unconfigured>, 20>,
    pub p21: Pin<PORT, Gpio<Unconfigured>, 21>,
    pub p22: Pin<PORT, Gpio<Unconfigured>, 22>,
    pub p23: Pin<PORT, Gpio<Unconfigured>, 23>,
    pub p24: Pin<PORT, Gpio<Unconfigured>, 24>,
    pub p25: Pin<PORT, Gpio<Unconfigured>, 25>,
    pub p26: Pin<PORT, Gpio<Unconfigured>, 26>,
    pub p27: Pin<PORT, Gpio<Unconfigured>, 27>,
    pub p28: Pin<PORT, Gpio<Unconfigured>, 28>,
    pub p29: Pin<PORT, Gpio<Unconfigured>, 29>,
    pub p30: Pin<PORT, Gpio<Unconfigured>, 30>,
    pub p31: Pin<PORT, Gpio<Unconfigured>, 31>,
}

impl<PORT> Pio<PORT>
where
    PORT: sealed::Port,
{
    /// Create a HAL representation for the given port.
    ///
    /// Also enabled the PMC clocks for the given port.
    pub fn new(periph: PORT, pmc: &mut Pmc) -> Result<Self, ()> {
        // Should be impossible, but okay.
        pmc.enable_peripherals(&[PORT::PID]).map_err(drop)?;

        Ok(Self { periph })
    }

    /// Split the port into Pins.
    pub fn split(self) -> SplitPort<PORT> {
        // Configure all pins to an expected state.

        // Disable port write protection
        self.periph.pio_wpmr.modify(|_r, w| {
            w.wpkey().passwd();
            w.wpen().clear_bit();
            w
        });

        // Set all pins to GPIO/"PIO" mode (not peripheral mode)
        self.periph
            .pio_per
            .write(|w| unsafe { w.bits(0xFFFF_FFFF) });

        // Disable "Multi Drive" mode
        self.periph
            .pio_mddr
            .write(|w| unsafe { w.bits(0xFFFF_FFFF) });

        // Disable Pull Up resistors
        self.periph
            .pio_pudr
            .write(|w| unsafe { w.bits(0xFFFF_FFFF) });

        // Disable Pull Down resistors
        self.periph
            .pio_ppddr
            .write(|w| unsafe { w.bits(0xFFFF_FFFF) });

        // Disable Output Write registers
        self.periph
            .pio_owdr
            .write(|w| unsafe { w.bits(0xFFFF_FFFF) });

        // Disable output mode
        self.periph
            .pio_odr
            .write(|w| unsafe { w.bits(0xFFFF_FFFF) });

        // Set initial output state to low
        self.periph
            .pio_odsr
            .write(|w| unsafe { w.bits(0x0000_0000) });

        // Set initial drive strength to low
        self.periph
            .pio_driver
            .write(|w| unsafe { w.bits(0x0000_0000) });

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
    use crate::pmc::PeripheralIdentifier;
    use crate::target_device::{pioa, PIOA, PIOB, PIOC, PIOD, PIOE};
    use core::ops::Deref;

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
