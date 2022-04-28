/// Watchdog Timer
///
/// Only supports disabling the WDT at this time.

use crate::target_device::WDT;

pub struct Wdt {
    periph: WDT,
}

impl Wdt {
    pub fn new(periph: WDT) -> Self {
        Self { periph }
    }

    pub fn disable(&mut self) {
        self.periph.wdt_mr.modify(|_r, w| w.wddis().set_bit());
    }
}
