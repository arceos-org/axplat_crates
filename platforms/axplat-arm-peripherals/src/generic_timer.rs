//! ARM Generic Timer.

use axcpu::generic_timer::{GenericTimer, PhysicalTimer as Timer};
use int_ratio::Ratio;

static mut CNTPCT_TO_NANOS_RATIO: Ratio = Ratio::zero();
static mut NANOS_TO_CNTPCT_RATIO: Ratio = Ratio::zero();

/// Returns the current clock time in hardware ticks.
#[inline]
pub fn current_ticks() -> u64 {
    Timer::counter()
}

/// Converts hardware ticks to nanoseconds.
#[inline]
pub fn ticks_to_nanos(ticks: u64) -> u64 {
    unsafe { CNTPCT_TO_NANOS_RATIO.mul_trunc(ticks) }
}

/// Converts nanoseconds to hardware ticks.
#[inline]
pub fn nanos_to_ticks(nanos: u64) -> u64 {
    unsafe { NANOS_TO_CNTPCT_RATIO.mul_trunc(nanos) }
}

/// Set a one-shot timer.
///
/// A timer interrupt will be triggered at the specified monotonic time deadline (in nanoseconds).
pub fn set_oneshot_timer(deadline_ns: u64) {
    let cur_ticks = current_ticks();
    let deadline_ticks = nanos_to_ticks(deadline_ns);
    if cur_ticks < deadline_ticks {
        let interval = deadline_ticks - cur_ticks;
        debug_assert!(interval <= u32::MAX as u64);
        Timer::set_countdown(interval as u32);
    } else {
        Timer::set_countdown(0);
    }
}

/// Early stage initialization: stores the timer frequency.
pub fn init_early() {
    let freq = Timer::frequency();
    unsafe {
        CNTPCT_TO_NANOS_RATIO = Ratio::new(axplat::time::NANOS_PER_SEC as u32, freq);
        NANOS_TO_CNTPCT_RATIO = CNTPCT_TO_NANOS_RATIO.inverse();
    }
}

/// Enable timer interrupts.
///
/// It should be called on all CPUs, as the timer interrupt is a PPI (Private
/// Peripheral Interrupt).
#[cfg(feature = "irq")]
pub fn enable_irqs(timer_irq_num: usize) {
    Timer::set_enable(true);
    Timer::set_countdown(0);
    axplat::irq::set_enable(timer_irq_num, true);
}

/// Default implementation of [`axplat::time::TimeIf`] using the generic
/// timer.
#[macro_export]
macro_rules! time_if_impl {
    ($name:ident) => {
        struct $name;

        #[impl_plat_interface]
        impl axplat::time::TimeIf for $name {
            /// Returns the current clock time in hardware ticks.
            fn current_ticks() -> u64 {
                $crate::generic_timer::current_ticks()
            }

            /// Converts hardware ticks to nanoseconds.
            fn ticks_to_nanos(ticks: u64) -> u64 {
                $crate::generic_timer::ticks_to_nanos(ticks)
            }

            /// Converts nanoseconds to hardware ticks.
            fn nanos_to_ticks(nanos: u64) -> u64 {
                $crate::generic_timer::nanos_to_ticks(nanos)
            }

            /// Return epoch offset in nanoseconds (wall time offset to monotonic
            /// clock start).
            fn epochoffset_nanos() -> u64 {
                $crate::pl031::epochoffset_nanos()
            }

            /// Set a one-shot timer.
            ///
            /// A timer interrupt will be triggered at the specified monotonic time
            /// deadline (in nanoseconds).
            #[cfg(feature = "irq")]
            fn set_oneshot_timer(deadline_ns: u64) {
                $crate::generic_timer::set_oneshot_timer(deadline_ns)
            }
        }
    };
}
