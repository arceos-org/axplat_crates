use axplat::irq::{HandlerTable, IpiTarget, IrqHandler, IrqIf};
use loongArch64::register::{
    ecfg::{self, LineBasedInterrupt},
    ticlr,
};

use crate::config::devices::{IPI_IRQ, TIMER_IRQ};

/// The maximum number of IRQs.
pub const MAX_IRQ_COUNT: usize = 0x13;
const IOCSR_IPI_SEND_CPU_SHIFT: u32 = 16;
const IOCSR_IPI_SEND_BLOCKING: u32 = 1 << 31;

const IOCSR_IPI_STATUS: u32 = 0x1000;
const IOCSR_IPI_ENABLE: u32 = 0x1004;
const IOCSR_IPI_CLEAR: u32 = 0x100c;
const IOCSR_IPI_SEND: u32 = 0x1040;

#[inline(always)]
fn read_iocsr(reg: u32) -> u32 {
    let val: u32;
    unsafe {
        core::arch::asm!(
            "iocsrrd.w {}, {}",
            out(reg) val,
            in(reg) reg,
            options(nostack, nomem)
        );
    }
    val
}

#[inline(always)]
fn write_iocsr(reg: u32, val: u32) {
    unsafe {
        core::arch::asm!(
            "iocsrwr.w {}, {}",
            in(reg) val,
            in(reg) reg,
            options(nostack)
        );
    }
}

static IRQ_HANDLER_TABLE: HandlerTable<MAX_IRQ_COUNT> = HandlerTable::new();

struct IrqIfImpl;

#[impl_plat_interface]
impl IrqIf for IrqIfImpl {
    /// Enables or disables the given IRQ.
    fn set_enable(irq_num: usize, enabled: bool) {
        let core_local_irq = match irq_num {
            TIMER_IRQ => Some(LineBasedInterrupt::TIMER),
            IPI_IRQ => {
                write_iocsr(IOCSR_IPI_ENABLE, u32::MAX);
                Some(LineBasedInterrupt::IPI)
            }
            _ => None,
        };

        if let Some(interrupt_bit) = core_local_irq {
            let old_value = ecfg::read().lie();
            let new_value = match enabled {
                true => old_value | interrupt_bit,
                false => old_value & !interrupt_bit,
            };
            ecfg::set_lie(new_value);
        }
    }

    /// Registers an IRQ handler for the given IRQ.
    fn register(irq_num: usize, handler: IrqHandler) -> bool {
        if IRQ_HANDLER_TABLE.register_handler(irq_num, handler) {
            Self::set_enable(irq_num, true);
            return true;
        }
        warn!("register handler for IRQ {} failed", irq_num);
        false
    }

    /// Unregisters the IRQ handler for the given IRQ.
    ///
    /// It also disables the IRQ if the unregistration succeeds. It returns the
    /// existing handler if it is registered, `None` otherwise.
    fn unregister(irq: usize) -> Option<IrqHandler> {
        Self::set_enable(irq, false);
        IRQ_HANDLER_TABLE.unregister_handler(irq)
    }

    /// Handles the IRQ.
    ///
    /// It is called by the common interrupt handler. It should look up in the
    /// IRQ handler table and calls the corresponding handler. If necessary, it
    /// also acknowledges the interrupt controller after handling.
    fn handle(irq: usize) {
        if irq == TIMER_IRQ {
            ticlr::clear_timer_interrupt();
        } else if irq == IPI_IRQ {
            write_iocsr(IOCSR_IPI_CLEAR, 0x1);
        }
        trace!("IRQ {}", irq);
        if !IRQ_HANDLER_TABLE.handle(irq) {
            warn!("Unhandled IRQ {}", irq);
        }
    }

    /// Sends an inter-processor interrupt (IPI) to the specified target CPU or all CPUs.
    fn send_ipi(_irq_num: usize, target: IpiTarget) {
        match target {
            IpiTarget::Current { cpu_id } => {
                write_iocsr(
                    IOCSR_IPI_SEND,
                    (cpu_id as u32) << IOCSR_IPI_SEND_CPU_SHIFT | IOCSR_IPI_SEND_BLOCKING,
                );
            }
            IpiTarget::Other { cpu_id } => {
                write_iocsr(
                    IOCSR_IPI_SEND,
                    (cpu_id as u32) << IOCSR_IPI_SEND_CPU_SHIFT | IOCSR_IPI_SEND_BLOCKING,
                );
            }
            IpiTarget::AllExceptCurrent { cpu_id, cpu_num } => {
                for i in 0..cpu_num {
                    if i != cpu_id {
                        write_iocsr(
                            IOCSR_IPI_SEND,
                            (i as u32) << IOCSR_IPI_SEND_CPU_SHIFT | IOCSR_IPI_SEND_BLOCKING,
                        );
                    }
                }
            }
        }
    }
}
