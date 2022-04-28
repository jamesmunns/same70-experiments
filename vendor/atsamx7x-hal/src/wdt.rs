//! Watchdog Timer
//!
//! Only supports disabling the WDT at this time.

use crate::target_device::WDT;

/// The HAL interface for the WDT peripheral.
///
/// NOTE: On startup, the watchdog timer is already running,
/// regardless of whether you create a `Wdt` struct or not!
pub struct Wdt {
    periph: WDT,
}

impl Wdt {
    /// Create a new Watchdog HAL interface
    pub fn new(periph: WDT) -> Self {
        Self { periph }
    }

    /// Disable the watchdog timer
    pub fn disable(&mut self) {
        self.periph.wdt_mr.modify(|_r, w| w.wddis().set_bit());
    }
}
