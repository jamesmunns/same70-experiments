/// Real Time Timer peripheral
///
/// This peripheral is hardcoded to tick a 32-bit timer at 8192Hz.
/// The resulting clock is used for general timing and rtic::Monotonic
/// purposes.
///
/// This timer will roll over approximately every 145 hours.

use core::sync::atomic::{AtomicBool, Ordering};
use groundhog::RollingTimer;
use crate::target_device::{RTT, rtt::RegisterBlock};
use rtic_monotonic::Monotonic;

pub struct Rtt {
    periph: RTT
}

impl Rtt {
    pub const TICK_SCALER: u32 = 4;
    pub const TICKS_PER_SEC: u32 = 32_768 / Rtt::TICK_SCALER;

    /// Create and start the RTT at the fixed frequency
    pub fn new(periph: RTT) -> Self {
        periph.rtt_mr.modify(|_r, w| {
            // Feed from 32khz prescaled val
            w.rtc1hz().clear_bit();

            // Enable
            w.rttdis().clear_bit();

            // Reset value
            w.rttrst().set_bit();

            // No roll-over interrupt
            w.rttincien().clear_bit();

            // No alarm
            w.almien().clear_bit();

            unsafe {
                // Set prescaler to four. We could use three, but four gives us an even
                // number of ticks per second. This gives a minimum resolution of ~122uS,
                // and a maximum range of ~145 hours
                w.rtpres().bits(Self::TICK_SCALER as u16);
            }

            w
        });

        // Enable use of Global Rolling Timer
        IS_GRT_INIT.store(true, Ordering::SeqCst);

        Rtt {
            periph
        }
    }

    /// Get the current 8192Hz tick since start (or last rollover)
    pub fn get_tick(&self) -> u32 {
        get_cur_tick(&self.periph)
    }

    /// Enable the ALARM interrupt to trigger at the requested tick.
    pub fn set_alarm_int(&mut self, tick: u32) {
        // Disable interrupt before changing alarm
        self.periph.rtt_mr.modify(|_r, w| {
            w.almien().clear_bit()
        });

        // Set new value
        self.periph.rtt_ar.write(|w| unsafe {
            // Note: interrupt is triggered when ALMV + 1 is reached.
            // Subtract one to trigger AT `tick` value
            w.bits(tick.wrapping_sub(1))
        });

        // Clear the interrupt flag by reading the status register
        // NOTE: If we want to support the roll-over interrupt, we
        // might need to be more careful here.
        let _ = self.periph.rtt_sr.read();

        // Enable interrupt
        self.periph.rtt_mr.modify(|_r, w| {
            w.almien().set_bit()
        });
    }

    /// Disable the alarm interrupt, and clear the alarm status flag
    pub fn clear_disable_alarm_int(&mut self) {
        // Disable interrupt
        self.periph.rtt_mr.modify(|_r, w| {
            w.almien().clear_bit()
        });

        // Clear the interrupt flag by reading the status register
        // NOTE: If we want to support the roll-over interrupt, we
        // might need to be more careful here.
        let _ = self.periph.rtt_sr.read();
    }
}

fn get_cur_tick(rtt: &RegisterBlock) -> u32 {
    let mut last = rtt.rtt_vr.read().bits();

    // This value is susceptible to read tearing. Read in a loop
    // to check that values match.
    loop {
        let new = rtt.rtt_vr.read().bits();
        if last == new {
            return last;
        }
        last = new;
    }
}

static IS_GRT_INIT: AtomicBool = AtomicBool::new(false);

/// A global rolling timer. NOTE: RTT MUST be enabled via
/// [`Rtt::new()`](Rtt::new()) first!
pub struct GlobalRollingTimer;

impl Default for GlobalRollingTimer {
    fn default() -> Self {
        GlobalRollingTimer
    }
}

impl Clone for GlobalRollingTimer {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl RollingTimer for GlobalRollingTimer {
    type Tick = u32;

    const TICKS_PER_SECOND: Self::Tick = Rtt::TICKS_PER_SEC;

    fn get_ticks(&self) -> Self::Tick {
        if !self.is_initialized() {
            return 0;
        }

        // SAFETY:
        //
        // Only atomic reads of the current value is used.
        let rtt = unsafe { &*RTT::ptr() };

        get_cur_tick(rtt)
    }

    fn is_initialized(&self) -> bool {
        IS_GRT_INIT.load(Ordering::SeqCst)
    }
}



impl Monotonic for Rtt {
    type Instant = fugit::TimerInstantU32<{ Self::TICKS_PER_SEC }>;
    type Duration = fugit::TimerDurationU32<{ Self::TICKS_PER_SEC }>;

    unsafe fn reset(&mut self) {
        // Don't do anything, just rely on the regular init
    }

    #[inline(always)]
    fn now(&mut self) -> Self::Instant {
        Self::Instant::from_ticks(self.get_tick())
    }

    fn set_compare(&mut self, instant: Self::Instant) {
        self.set_alarm_int(instant.duration_since_epoch().ticks())
    }

    fn clear_compare_flag(&mut self) {
        self.clear_disable_alarm_int();
    }

    #[inline(always)]
    fn zero() -> Self::Instant {
        Self::Instant::from_ticks(0)
    }
}
