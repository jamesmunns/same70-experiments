#![no_main]
#![no_std]

use defmt_rtt as _; // global logger
pub use atsamx7x_hal as hal; // memory layout
use panic_probe as _;
use hal::GlobalRollingTimer;
use groundhog::RollingTimer;


// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

/// Terminates the application and makes `probe-run` exit with exit-code = 0
pub fn exit() -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

defmt::timestamp!("{=f32}", {
    (GlobalRollingTimer::default().get_ticks() as f32) / (32768.0 / 4.0)
});
