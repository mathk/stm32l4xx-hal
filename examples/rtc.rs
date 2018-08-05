//! Blinks an LED

#![deny(unsafe_code)]
// #![deny(warnings)]
#![no_std]
#![no_main]

extern crate cortex_m;
#[macro_use]
extern crate cortex_m_rt as rt;
extern crate cortex_m_semihosting as sh;
extern crate panic_semihosting;
extern crate stm32l432xx_hal as hal;
// #[macro_use(block)]
// extern crate nb;

use hal::prelude::*;
use hal::stm32l4::stm32l4x2;

use hal::delay::Delay;
use hal::rtc::Rtc;
use hal::pwr::Pwr;
use hal::datetime::{Date,Time};
use rt::ExceptionFrame;

use core::fmt::Write;
use sh::hio;

entry!(main);

fn main() -> ! {

    let mut hstdout = hio::hstdout().unwrap();

    writeln!(hstdout, "Hello, world!").unwrap();

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32l4x2::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain(); // .constrain();
    let mut rcc = dp.RCC.constrain();

    // Try a different clock configuration
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    // let clocks = rcc.cfgr
    //     .sysclk(64.mhz())
    //     .pclk1(32.mhz())
    //     .freeze(&mut flash.acr);
    let mut timer = Delay::new(cp.SYST, clocks);
    let mut pwr = Pwr::pwr(&mut rcc.apb1r1);
    let rtc = Rtc::rtc(dp.RTC, &mut rcc.apb1r1, &mut rcc.bdcr, &mut pwr.cr1);
    
    let mut time = Time::new(21.hours(), 57.minutes(), 32.seconds(), false);
    let mut date = Date::new(1.day(), 24.date(), 4.month(), 2018.year());
    
    rtc.set_time(&time);
    rtc.set_date(&date);

    timer.delay_ms(1000_u32);
    timer.delay_ms(1000_u32);
    timer.delay_ms(1000_u32);

    time = rtc.get_time();
    date = rtc.get_date();

    
    writeln!(hstdout, "Good bye!").unwrap();
    loop {}
}

exception!(HardFault, hard_fault);

fn hard_fault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}

exception!(*, default_handler);

fn default_handler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}