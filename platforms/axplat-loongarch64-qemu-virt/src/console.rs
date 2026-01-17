use crate::mem::mmio_phys_to_virt;
use axplat::mem::{PhysAddr, pa};
use kspin::SpinNoIrq;
use ns16550a::{
    Break, DMAMode, Divisor, ParityBit, ParitySelect, StickParity, StopBits, Uart, WordLength,
};

const UART_BASE: PhysAddr = pa!(crate::config::devices::UART_PADDR);

static UART: SpinNoIrq<Uart> = SpinNoIrq::new(Uart::new(mmio_phys_to_virt(UART_BASE).as_usize()));

/// Initializes the UART.
/// Note: QEMU's ns16550a UART is already initialized by firmware, so this
/// function is typically not needed. It's kept here for reference or for
/// cases where explicit initialization is required.
#[allow(dead_code)]
pub fn init() {
    UART.lock().init(
        WordLength::EIGHT,
        StopBits::ONE,
        ParityBit::DISABLE,
        ParitySelect::EVEN,
        StickParity::DISABLE,
        Break::DISABLE,
        DMAMode::MODE0,
        Divisor::BAUD115200,
    );
}

use axplat::console::ConsoleIf;

struct ConsoleIfImpl;

#[impl_plat_interface]
impl ConsoleIf for ConsoleIfImpl {
    /// Writes bytes to the console from input u8 slice.
    fn write_bytes(bytes: &[u8]) {
        for &c in bytes {
            let uart = UART.lock();
            match c {
                b'\n' => {
                    let _ = uart.put(b'\r');
                    let _ = uart.put(b'\n');
                }
                c => {
                    let _ = uart.put(c);
                }
            }
        }
    }

    /// Reads bytes from the console into the given mutable slice.
    /// Returns the number of bytes read.
    fn read_bytes(bytes: &mut [u8]) -> usize {
        for (i, byte) in bytes.iter_mut().enumerate() {
            match UART.lock().get() {
                Some(c) => *byte = c,
                None => return i,
            }
        }
        bytes.len()
    }
}
