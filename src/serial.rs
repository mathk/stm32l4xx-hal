//! Serial

use core::marker::{PhantomData, Unsize};
use core::ptr;
use core::sync::atomic::{self, Ordering};

use hal::serial;
use nb;
use stm32l4::stm32l4x2::{USART1, USART2};
use void::Void;

use gpio::gpioa::{PA10, PA2, PA3, PA9};
use gpio::gpiob::{PB6, PB7};
use gpio::AF7;
use rcc::{APB1R1, APB2, Clocks};
use time::Bps;
use dma::{dma1, CircBuffer, Static, Transfer, R, W};

/// Interrupt event
pub enum Event {
    /// New data has been received
    Rxne,
    /// New data can be sent
    Txe,
}

/// Serial error
#[derive(Debug)]
pub enum Error {
    /// Framing error
    Framing,
    /// Noise error
    Noise,
    /// RX buffer overrun
    Overrun,
    /// Parity check error
    Parity,
    #[doc(hidden)]
    _Extensible,
}

pub trait Pins<USART> {
    const REMAP: u8;
}

impl Pins<USART1> for (PA9<AF7>, PA10<AF7>) {
    const REMAP: u8 = 0;
}

impl Pins<USART1> for (PB6<AF7>, PB7<AF7>) {
    const REMAP: u8 = 1;
}

impl Pins<USART2> for (PA2<AF7>, PA3<AF7>) {
    const REMAP: u8 = 0;
}

// impl Pins<USART2> for (PD5<Alternate<PushPull>>, PD6<Input<Floating>>) {
//     const REMAP: u8 = 0;
// }


/// Serial abstraction
pub struct Serial<USART, PINS> {
    usart: USART,
    pins: PINS,
}

/// Serial receiver
pub struct Rx<USART> {
    _usart: PhantomData<USART>,
}

/// Serial transmitter
pub struct Tx<USART> {
    _usart: PhantomData<USART>,
}

macro_rules! hal {
    ($(
        $USARTX:ident: ($usartX:ident, $APB:ident, $usartXen:ident, $usartXrst:ident, $pclkX:ident, tx: $rx_chan:path, rx: $tx_chan:path),
    )+) => {
        $(
            impl<PINS> Serial<$USARTX, PINS> {
                /// Configures a USART peripheral to provide serial communication
                pub fn $usartX(
                    usart: $USARTX,
                    pins: PINS,
                    baud_rate: Bps,
                    clocks: Clocks,
                    apb: &mut $APB,
                ) -> Self
                where
                    PINS: Pins<$USARTX>,
                {
                    // enable or reset $USARTX
                    apb.enr().modify(|_, w| w.$usartXen().set_bit());
                    apb.rstr().modify(|_, w| w.$usartXrst().set_bit());
                    apb.rstr().modify(|_, w| w.$usartXrst().clear_bit());

                    // TODO implement pin remaping

                    // disable hardware flow control
                    // TODO enable DMA
                    // usart.cr3.write(|w| w.rtse().clear_bit().ctse().clear_bit());

                    let brr = clocks.$pclkX().0 / baud_rate.0;
                    assert!(brr >= 16, "impossible baud rate");
                    usart.brr.write(|w| unsafe { w.bits(brr) });

                    // UE: enable USART
                    // RE: enable receiver
                    // TE: enable transceiver
                    usart
                        .cr1
                        .write(|w| w.ue().set_bit().re().set_bit().te().set_bit());

                    Serial { usart, pins }
                }

                /// Starts listening for an interrupt event
                pub fn listen(&mut self, event: Event) {
                    match event {
                        Event::Rxne => {
                            self.usart.cr1.modify(|_, w| w.rxneie().set_bit())
                        },
                        Event::Txe => {
                            self.usart.cr1.modify(|_, w| w.txeie().set_bit())
                        },
                    }
                }

                /// Starts listening for an interrupt event
                pub fn unlisten(&mut self, event: Event) {
                    match event {
                        Event::Rxne => {
                            self.usart.cr1.modify(|_, w| w.rxneie().clear_bit())
                        },
                        Event::Txe => {
                            self.usart.cr1.modify(|_, w| w.txeie().clear_bit())
                        },
                    }
                }

                /// Splits the `Serial` abstraction into a transmitter and a receiver half
                pub fn split(self) -> (Tx<$USARTX>, Rx<$USARTX>) {
                    (
                        Tx {
                            _usart: PhantomData,
                        },
                        Rx {
                            _usart: PhantomData,
                        },
                    )
                }

                /// Releases the USART peripheral and associated pins
                pub fn free(self) -> ($USARTX, PINS) {
                    (self.usart, self.pins)
                }
            }

            impl serial::Read<u8> for Rx<$USARTX> {
                type Error = Error;

                fn read(&mut self) -> nb::Result<u8, Error> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$USARTX::ptr()).isr.read() };

                    Err(if isr.pe().bit_is_set() {
                        nb::Error::Other(Error::Parity)
                    } else if isr.fe().bit_is_set() {
                        nb::Error::Other(Error::Framing)
                    } else if isr.nf().bit_is_set() {
                        nb::Error::Other(Error::Noise)
                    } else if isr.ore().bit_is_set() {
                        nb::Error::Other(Error::Overrun)
                    } else if isr.rxne().bit_is_set() {
                        // NOTE(read_volatile) see `write_volatile` below
                        return Ok(unsafe {
                            ptr::read_volatile(&(*$USARTX::ptr()).rdr as *const _ as *const _)
                        });
                    } else {
                        nb::Error::WouldBlock
                    })
                }
            }

            impl serial::Write<u8> for Tx<$USARTX> {
                // NOTE(Void) See section "29.7 USART interrupts"; the only possible errors during
                // transmission are: clear to send (which is disabled in this case) errors and
                // framing errors (which only occur in SmartCard mode); neither of these apply to
                // our hardware configuration
                type Error = Void;

                fn flush(&mut self) -> nb::Result<(), Void> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$USARTX::ptr()).isr.read() };

                    if isr.tc().bit_is_set() {
                        Ok(())
                    } else {
                        Err(nb::Error::WouldBlock)
                    }
                }

                fn write(&mut self, byte: u8) -> nb::Result<(), Void> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$USARTX::ptr()).isr.read() };

                    if isr.txe().bit_is_set() {
                        // NOTE(unsafe) atomic write to stateless register
                        // NOTE(write_volatile) 8-bit write that's not possible through the svd2rust API
                        unsafe {
                            ptr::write_volatile(&(*$USARTX::ptr()).tdr as *const _ as *mut _, byte)
                        }
                        Ok(())
                    } else {
                        Err(nb::Error::WouldBlock)
                    }
                }
            }

            impl Rx<$USARTX> {
                pub fn circ_read<B>(
                    self,
                    mut chan: $rx_chan,
                    buffer: &'static mut [B; 2],
                ) -> CircBuffer<B, $rx_chan>
                where
                    B: Unsize<[u8]>,
                {
                    {
                        let buffer: &[u8] = &buffer[0];
                        chan.cmar().write(|w| unsafe {
                            w.ma().bits(buffer.as_ptr() as usize as u32)
                        });
                        chan.cndtr().write(|w| unsafe{
                            w.ndt().bits((buffer.len() * 2) as u16)
                        });
                        chan.cpar().write(|w| unsafe {
                            w.pa().bits(&(*$USARTX::ptr()).rdr as *const _ as usize as u32)
                        });

                        // TODO can we weaken this compiler barrier?
                        // NOTE(compiler_fence) operations on `buffer` should not be reordered after
                        // the next statement, which starts the DMA transfer
                        atomic::compiler_fence(Ordering::SeqCst);

                        chan.ccr().modify(|_, w| unsafe {
                            w.mem2mem()
                                .clear_bit()
                                // 00: Low, 01: Medium, 10: High, 11: Very high
                                .pl()
                                .bits(0b10)
                                // 00: 8-bits, 01: 16-bits, 10: 32-bits, 11: Reserved
                                .msize()
                                .bits(0b00)
                                // 00: 8-bits, 01: 16-bits, 10: 32-bits, 11: Reserved
                                .psize()
                                .bits(0b00)
                                .minc()
                                .set_bit()
                                .pinc()
                                .clear_bit()
                                .circ()
                                .set_bit()
                                .dir()
                                .clear_bit()
                                .en()
                                .set_bit()
                        });
                    }

                    CircBuffer::new(buffer, chan)
                }
            }
        )+
    }
}

hal! {
    USART1: (usart1, APB2, usart1en, usart1rst, pclk2, tx: dma1::C4, rx: dma1::C5),
    USART2: (usart2, APB1R1, usart2en, usart2rst, pclk1, tx: dma1::C7, rx: dma1::C6),
}
