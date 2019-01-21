//! Test the serial interface
//!
#![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

extern crate cortex_m;
#[macro_use(entry, exception)]
extern crate cortex_m_rt as rt;
#[macro_use(block)]
extern crate nb;
extern crate panic_semihosting;

extern crate stm32l4xx_hal as hal;
extern crate mfxstm32l152 as mfx;
// #[macro_use(block)]
// extern crate nb;

use cortex_m::asm;
use crate::hal::prelude::*;
use crate::hal::serial::Serial;
use crate::hal::delay::Delay;
use crate::hal::i2c::I2c;
use crate::rt::ExceptionFrame;
use mfx::MFX;

use core::fmt::{self, Write};

struct Wrapper<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> Wrapper<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Wrapper {
            buf: buf,
            offset: 0,
        }
    }
}

impl<'a> fmt::Write for Wrapper<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();

        // Skip over already-copied data
        let remainder = &mut self.buf[self.offset..];
        // Check if there is space remaining (return error instead of panicking)
        if remainder.len() < bytes.len() { return Err(core::fmt::Error); }
        // Make the two slices the same length
        let remainder = &mut remainder[..bytes.len()];
        // Copy
        remainder.copy_from_slice(bytes);

        // Update offset to avoid overwriting
        self.offset += bytes.len();

        Ok(())
    }
}

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let p = hal::stm32::Peripherals::take().unwrap();

    let mut flash = p.FLASH.constrain();
    let mut rcc = p.RCC.constrain();
    // let mut gpioa = p.GPIOA.split(&mut rcc.ahb2);
    // let mut gpiob = p.GPIOB.split(&mut rcc.ahb2);
    let mut gpiod = p.GPIOD.split(&mut rcc.ahb2);
    let mut gpiob = p.GPIOB.split(&mut rcc.ahb2);
    let mut gpioa = p.GPIOA.split(&mut rcc.ahb2);

    // clock configuration using the default settings (all clocks run at 8 MHz)
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    // TRY this alternate clock configuration (clocks run at nearly the maximum frequency)
    // let clocks = rcc.cfgr.sysclk(64.mhz()).pclk1(32.mhz()).freeze(&mut flash.acr);


    //let tx = gpioa.pa2.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
    // let tx = gpiob.pb6.into_af7(&mut gpiob.moder, &mut gpiob.afrl);
    let tx = gpiod.pd5.into_af7(&mut gpiod.moder, &mut gpiod.afrl);

    // let rx = gpioa.pa3.into_af7(&mut gpioa.moder, &mut gpioa.afrl);
    // let rx = gpiob.pb7.into_af7(&mut gpiob.moder, &mut gpiob.afrl);
    let rx = gpiod.pd6.into_af7(&mut gpiod.moder, &mut gpiod.afrl);

    let mut scl = gpiob.pb10.into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
    scl.internal_pull_up(&mut gpiob.pupdr, true);
    let scl = scl.into_af4(&mut gpiob.moder, &mut gpiob.afrh);

    let mut sda = gpiob.pb11.into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
    sda.internal_pull_up(&mut gpiob.pupdr, true);
    let sda = sda.into_af4(&mut gpiob.moder, &mut gpiob.afrh);
    let wakup = gpioa.pa4.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

    let i2c = I2c::i2c2(p.I2C2, (scl, sda), 100.khz(), clocks, &mut rcc.apb1r1);
    let timer = Delay::new(cp.SYST, clocks);
    let mut mfx = MFX::new(i2c, wakup, timer, 0x84).unwrap();




     // TRY using a different USART peripheral here
    let serial = Serial::usart2(p.USART2, (tx, rx), 115_200.bps(), clocks, &mut rcc.apb1r1);
    let (mut tx, mut rx) = serial.split();

    let sent = b'X';



    // The `block!` macro makes an operation block until it finishes
    // NOTE the error type is `!`

    let shunt = mfx.last_shunt_used().unwrap();
    let mut buf = [0 as u8; 20];
    write!(Wrapper::new(&mut buf), "Shunt: {}", shunt).unwrap();

    tx.write_str(core::str::from_utf8(&buf).unwrap()).ok();
    block!(tx.write(sent)).ok();
    block!(tx.write(sent)).ok();
    block!(tx.write(sent)).ok();
    block!(tx.write(sent)).ok();
    block!(tx.write(sent)).ok();

    // when using virtual com port for recieve can causes a framing error
    // On the stm32l476 discovery it is working fine at 115200 baud
    let received = block!(rx.read()).unwrap();

    assert_eq!(received, sent);

    // if all goes well you should reach this breakpoint
    asm::bkpt();

    loop {}
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
