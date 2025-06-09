//! TODO: PLIC

use axplat::irq::{HandlerTable, IrqHandler, IrqIf};
use lazyinit::LazyInit;
use riscv::register::sie;

/// Supervisor software interrupt in `scause`
#[allow(unused)]
pub(super) const S_SOFT: usize = 1;

/// Supervisor timer interrupt in `scause`
pub(super) const S_TIMER: usize = 5;

/// Supervisor external interrupt in `scause`
pub(super) const S_EXT: usize = 9;

static TIMER_HANDLER: LazyInit<IrqHandler> = LazyInit::new();

/// The maximum number of IRQs.
pub const MAX_IRQ_COUNT: usize = 1024;

static IRQ_HANDLER_TABLE: HandlerTable<MAX_IRQ_COUNT> = HandlerTable::new();

macro_rules! with_cause {
    ($cause: expr, @TIMER => $timer_op: expr, @EXT => $ext_op: expr $(,)?) => {
        match $cause {
            S_TIMER => $timer_op,
            S_EXT => $ext_op,
            _ => panic!("invalid trap cause: {:#x}", $cause),
        }
    };
}

pub(super) fn init_percpu() {
    // enable soft interrupts, timer interrupts, and external interrupts
    unsafe {
        sie::set_ssoft();
        sie::set_stimer();
        sie::set_sext();
    }
}

struct IrqIfImpl;

#[impl_plat_interface]
impl IrqIf for IrqIfImpl {
    /// Enables or disables the given IRQ.
    fn set_enable(irq: usize, _enabled: bool) {
        if irq == S_EXT {
            // TODO: set enable in PLIC
        }
    }

    /// Registers an IRQ handler for the given IRQ.
    ///
    /// It also enables the IRQ if the registration succeeds. It returns `false` if
    /// the registration failed.
    fn register(irq: usize, handler: IrqHandler) -> bool {
        with_cause!(
            irq,
            @TIMER => if !TIMER_HANDLER.is_inited() {
                TIMER_HANDLER.init_once(handler);
                true
            } else {
                false
            },
            @EXT => if IRQ_HANDLER_TABLE.register_handler(irq, handler) {
                Self::set_enable(irq, true);
                return true;
            } else {
                warn!("register handler for IRQ {} failed", irq);
                false
            }
        )
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
        with_cause!(
            irq,
            @TIMER => {
                trace!("IRQ: timer");
                TIMER_HANDLER();
            },
            @EXT => {
                trace!("IRQ: external");
                if !IRQ_HANDLER_TABLE.handle(irq) {
                    warn!("Unhandled IRQ {}", irq);
                }
            }
        );
    }
}
