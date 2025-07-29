//! ARM Generic Interrupt Controller (GIC).

use arm_gicv2::{GicCpuInterface, GicDistributor, InterruptType, translate_irq};
use axplat::irq::{HandlerTable, IrqHandler};
use axplat::mem::VirtAddr;
use kspin::SpinNoIrq;
use lazyinit::LazyInit;

/// The maximum number of IRQs.
const MAX_IRQ_COUNT: usize = 1024;

static GICD: LazyInit<SpinNoIrq<GicDistributor>> = LazyInit::new();

// per-CPU, no lock
static GICC: LazyInit<GicCpuInterface> = LazyInit::new();

static IRQ_HANDLER_TABLE: HandlerTable<MAX_IRQ_COUNT> = HandlerTable::new();

/// Enables or disables the given IRQ.
pub fn set_enable(irq_num: usize, enabled: bool) {
    trace!("GICD set enable: {} {}", irq_num, enabled);
    GICD.lock().set_enable(irq_num as _, enabled);
}

/// Registers an IRQ handler for the given IRQ.
///
/// It also enables the IRQ if the registration succeeds. It returns `false`
/// if the registration failed.
pub fn register_handler(irq_num: usize, handler: IrqHandler) -> bool {
    trace!("register handler IRQ {}", irq_num);
    if IRQ_HANDLER_TABLE.register_handler(irq_num, handler) {
        set_enable(irq_num, true);
        return true;
    }
    warn!("register handler for IRQ {} failed", irq_num);
    false
}

/// Unregisters the IRQ handler for the given IRQ.
///
/// It also disables the IRQ if the unregistration succeeds. It returns the
/// existing handler if it is registered, `None` otherwise.
pub fn unregister_handler(irq_num: usize) -> Option<IrqHandler> {
    trace!("unregister handler IRQ {}", irq_num);
    set_enable(irq_num, false);
    IRQ_HANDLER_TABLE.unregister_handler(irq_num)
}

/// Handles the IRQ.
///
/// It is called by the common interrupt handler. It should look up in the
/// IRQ handler table and calls the corresponding handler. If necessary, it
/// also acknowledges the interrupt controller after handling.
pub fn handle_irq(_unused: usize) {
    GICC.handle_irq(|irq_num| {
        trace!("IRQ {}", irq_num);
        if !IRQ_HANDLER_TABLE.handle(irq_num as _) {
            warn!("Unhandled IRQ {}", irq_num);
        }
    });
}

/// Returns the IRQ number of the IPI.
pub fn get_ipi_irq_num() -> usize {
    translate_irq(1, InterruptType::SGI).unwrap()
}

/// Sends Software Generated Interrupt (SGI)(s) (usually IPI) to the given dest CPU.
pub fn send_ipi_one(dest_cpu_id: usize, irq_num: usize) {
    GICD.lock().send_sgi(dest_cpu_id, irq_num);
}

/// Sends a broadcast IPI to all CPUs.
pub fn send_ipi_all_others(irq_num: usize, _src_cpu_id: usize, _cpu_num: usize) {
    GICD.lock().send_sgi_all_except_self(irq_num);
}

/// Initializes GICD (for the primary CPU only).
pub fn init_gicd(gicd_base: VirtAddr, gicc_base: VirtAddr) {
    info!("Initialize GICv2...");
    GICD.init_once(SpinNoIrq::new(GicDistributor::new(gicd_base.as_mut_ptr())));
    GICC.init_once(GicCpuInterface::new(gicc_base.as_mut_ptr()));
    GICD.lock().init();
}

/// Initializes GICC (for all CPUs).
///
/// It must be called after [`init_gicd`].
pub fn init_gicc() {
    GICC.init();
}

/// Default implementation of [`axplat::irq::IrqIf`] using the GIC.
#[macro_export]
macro_rules! irq_if_impl {
    ($name:ident) => {
        struct $name;

        #[impl_plat_interface]
        impl axplat::irq::IrqIf for $name {
            /// Enables or disables the given IRQ.
            fn set_enable(irq: usize, enabled: bool) {
                $crate::gic::set_enable(irq, enabled);
            }

            /// Registers an IRQ handler for the given IRQ.
            ///
            /// It also enables the IRQ if the registration succeeds. It returns `false`
            /// if the registration failed.
            fn register(irq: usize, handler: axplat::irq::IrqHandler) -> bool {
                $crate::gic::register_handler(irq, handler)
            }

            /// Unregisters the IRQ handler for the given IRQ.
            ///
            /// It also disables the IRQ if the unregistration succeeds. It returns the
            /// existing handler if it is registered, `None` otherwise.
            fn unregister(irq: usize) -> Option<axplat::irq::IrqHandler> {
                $crate::gic::unregister_handler(irq)
            }

            /// Handles the IRQ.
            ///
            /// It is called by the common interrupt handler. It should look up in the
            /// IRQ handler table and calls the corresponding handler. If necessary, it
            /// also acknowledges the interrupt controller after handling.
            fn handle(irq: usize) {
                $crate::gic::handle_irq(irq)
            }

            /// Returns the IRQ number of the IPI.
            fn get_ipi_irq_num() -> usize {
                $crate::gic::get_ipi_irq_num()
            }

            /// Sends Software Generated Interrupt (SGI)(s) (usually IPI) to the given dest CPU.
            fn send_ipi_one(dest_cpu_id: usize, irq_num: usize) {
                $crate::gic::send_ipi_one(dest_cpu_id, irq_num);
            }

            /// Sends a broadcast IPI to all CPUs.
            fn send_ipi_all_others(irq_num: usize, _src_cpu_id: usize, _cpu_num: usize) {
                $crate::gic::send_ipi_all_others(irq_num, _src_cpu_id, _cpu_num);
            }
        }
    };
}
