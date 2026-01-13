//! Interrupt handling for RISC-V with PLIC support

use axplat::irq::{HandlerTable, IpiTarget, IrqHandler, IrqIf};
use core::sync::atomic::{AtomicPtr, Ordering};
use riscv::register::sie;
use sbi_rt::HartMask;

/// `Interrupt` bit in `scause`
pub(super) const INTC_IRQ_BASE: usize = 1 << (usize::BITS - 1);

/// Supervisor software interrupt in `scause`
#[allow(unused)]
pub(super) const S_SOFT: usize = INTC_IRQ_BASE + 1;

/// Supervisor timer interrupt in `scause`
pub(super) const S_TIMER: usize = INTC_IRQ_BASE + 5;

/// Supervisor external interrupt in `scause`
pub(super) const S_EXT: usize = INTC_IRQ_BASE + 9;

static TIMER_HANDLER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

static IPI_HANDLER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// The maximum number of IRQs.
pub const MAX_IRQ_COUNT: usize = 1024;

static IRQ_HANDLER_TABLE: HandlerTable<MAX_IRQ_COUNT> = HandlerTable::new();

macro_rules! with_cause {
    ($cause: expr, @S_TIMER => $timer_op: expr, @S_SOFT => $ipi_op: expr, @S_EXT => $ext_op: expr, @EX_IRQ => $plic_op: expr $(,)?) => {
        match $cause {
            S_TIMER => $timer_op,
            S_SOFT => $ipi_op,
            S_EXT => $ext_op,
            other => {
                if other & INTC_IRQ_BASE == 0 {
                    // Device-side interrupts read from PLIC
                    $plic_op
                } else {
                    // Other CPU-side interrupts
                    panic!("Unknown IRQ cause: {}", other);
                }
            }
        }
    };
}

pub(super) fn init_percpu() {
    // Initialize PLIC
    if let Err(e) = crate::plic::init() {
        warn!("Failed to initialize PLIC: {:?}", e);
    }
    
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
    fn set_enable(irq: usize, enabled: bool) {
        if irq & INTC_IRQ_BASE == 0 {
            // Device-side interrupt - use PLIC
            let plic = crate::plic::get();
            if enabled {
                if let Err(e) = plic.enable_interrupt(0, irq) { // Context 0 for supervisor mode
                    warn!("Failed to enable interrupt {}: {:?}", irq, e);
                }
            } else if let Err(e) = plic.disable_interrupt(0, irq) {
                warn!("Failed to disable interrupt {}: {:?}", irq, e);
            }
        } else {
            // CPU-side interrupt - handled by CPU directly
            match irq {
                S_TIMER => {
                    if enabled {
                        unsafe { sie::set_stimer(); }
                    } else {
                        unsafe { sie::clear_stimer(); }
                    }
                }
                S_SOFT => {
                    if enabled {
                        unsafe { sie::set_ssoft(); }
                    } else {
                        unsafe { sie::clear_ssoft(); }
                    }
                }
                S_EXT => {
                    if enabled {
                        unsafe { sie::set_sext(); }
                    } else {
                        unsafe { sie::clear_sext(); }
                    }
                }
                _ => {
                    warn!("Unknown CPU-side IRQ: {}", irq);
                }
            }
        }
    }

    /// Registers an IRQ handler for the given IRQ.
    ///
    /// It also enables the IRQ if the registration succeeds. It returns `false` if
    /// the registration failed.
    ///
    /// The `irq` parameter has the following semantics
    /// 1. If its highest bit is 1, it means it is an interrupt on the CPU side. Its
    /// value comes from `scause`, where [`S_SOFT`] represents software interrupt
    /// and [`S_TIMER`] represents timer interrupt. If its value is [`S_EXT`], it
    /// means it is an external interrupt, and the real IRQ number needs to
    /// be obtained from PLIC.
    /// 2. If its highest bit is 0, it means it is an interrupt on the device side,
    /// and its value is equal to the IRQ number provided by PLIC.
    fn register(irq: usize, handler: IrqHandler) -> bool {
        with_cause!(
            irq,
            @S_TIMER => TIMER_HANDLER.compare_exchange(core::ptr::null_mut(), handler as *mut _, Ordering::AcqRel, Ordering::Acquire).is_ok(),
            @S_SOFT => IPI_HANDLER.compare_exchange(core::ptr::null_mut(), handler as *mut _, Ordering::AcqRel, Ordering::Acquire).is_ok(),
            @S_EXT => {
                warn!("External IRQ should be got from PLIC, not scause");
                false
            },
            @EX_IRQ => {
                if IRQ_HANDLER_TABLE.register_handler(irq, handler) {
                    Self::set_enable(irq, true);
                    true
                } else {
                    warn!("register handler for External IRQ {} failed", irq);
                    false
                }
            }
        )
    }

    /// Unregisters the IRQ handler for the given IRQ.
    ///
    /// It also disables the IRQ if the unregistration succeeds. It returns the
    /// existing handler if it is registered, `None` otherwise.
    fn unregister(irq: usize) -> Option<IrqHandler> {
        with_cause!(
            irq,
            @S_TIMER => {
                let handler = TIMER_HANDLER.swap(core::ptr::null_mut(), Ordering::AcqRel);
                if !handler.is_null() {
                    Some(unsafe { core::mem::transmute::<*mut (), IrqHandler>(handler) })
                } else {
                    None
                }
            },
            @S_SOFT => {
                let handler = IPI_HANDLER.swap(core::ptr::null_mut(), Ordering::AcqRel);
                if !handler.is_null() {
                    Some(unsafe { core::mem::transmute::<*mut (), IrqHandler>(handler) })
                } else {
                    None
                }
            },
            @S_EXT => {
                warn!("External IRQ should be got from PLIC, not scause");
                None
            },
            @EX_IRQ => IRQ_HANDLER_TABLE.unregister_handler(irq)
        )
    }

    /// Handles the IRQ.
    ///
    /// It is called by the common interrupt handler. It should look up in the
    /// IRQ handler table and calls the corresponding handler. If necessary, it
    /// also acknowledges the interrupt controller after handling.
    fn handle(irq: usize) {
        with_cause!(
            irq,
            @S_TIMER => {
                trace!("IRQ: timer");
                let handler = TIMER_HANDLER.load(Ordering::Acquire);
                if !handler.is_null() {
                    // SAFETY: The handler is guaranteed to be a valid function pointer.
                    unsafe { core::mem::transmute::<*mut (), IrqHandler>(handler)() };
                }
            },
            @S_SOFT => {
                trace!("IRQ: IPI");
                let handler = IPI_HANDLER.load(Ordering::Acquire);
                if !handler.is_null() {
                    // SAFETY: The handler is guaranteed to be a valid function pointer.
                    unsafe { core::mem::transmute::<*mut (), IrqHandler>(handler)() };
                }
                unsafe {
                    riscv::register::sip::clear_ssoft();
                }
            },
            @S_EXT => {
                // Get IRQ number from PLIC
                let plic = crate::plic::get();
                match plic.claim(0) {
                    Ok(Some(irq_num)) => {
                        if !IRQ_HANDLER_TABLE.handle(irq_num) {
                            warn!("Unhandled external IRQ {}", irq_num);
                        }
                        if let Err(e) = plic.complete(0, irq_num) {
                            warn!("Failed to complete interrupt {}: {:?}", irq_num, e);
                        }
                    }
                    Ok(None) => {
                        // No interrupt to claim
                    }
                    Err(e) => {
                        warn!("Failed to claim interrupt: {:?}", e);
                    }
                }
            },
            @EX_IRQ => {
                // Device-side IRQs are handled directly through the handler table
                // This should not be reached in normal operation, as device-side
                // interrupts should trigger the External Interrupt (S_EXT) which
                // then claims the interrupt from PLIC and calls the handler.
                if !IRQ_HANDLER_TABLE.handle(irq) {
                    warn!("Unhandled device-side IRQ {}", irq);
                }
            }
        )
    }

    /// Sends an inter-processor interrupt (IPI) to the specified target CPU or all CPUs.
    fn send_ipi(_irq_num: usize, target: IpiTarget) {
        match target {
            IpiTarget::Current { cpu_id } => {
                let res = sbi_rt::send_ipi(HartMask::from_mask_base(1 << cpu_id, 0));
                if res.is_err() {
                    warn!("send_ipi failed: {:?}", res);
                }
            }
            IpiTarget::Other { cpu_id } => {
                let res = sbi_rt::send_ipi(HartMask::from_mask_base(1 << cpu_id, 0));
                if res.is_err() {
                    warn!("send_ipi failed: {:?}", res);
                }
            }
            IpiTarget::AllExceptCurrent { cpu_id, cpu_num } => {
                for i in 0..cpu_num {
                    if i != cpu_id {
                        let res = sbi_rt::send_ipi(HartMask::from_mask_base(1 << i, 0));
                        if res.is_err() {
                            warn!("send_ipi_all_others failed: {:?}", res);
                        }
                    }
                }
            }
        }
    }
}
