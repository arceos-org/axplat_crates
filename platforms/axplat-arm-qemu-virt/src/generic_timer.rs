use core::arch::asm;

const NANOS_PER_SEC: u64 = 1_000_000_000;

struct TimeIfImpl;

/// Read the CNTFRQ register (Counter-timer Frequency register)
#[inline]
fn timer_frequency() -> u64 {
    let freq: u32;
    unsafe {
        // CNTFRQ: c14, c0, 0
        asm!("mrc p15, 0, {}, c14, c0, 0", out(reg) freq);
    }
    freq as u64
}

/// Read the CNTVCT register (Counter-timer Virtual Count register)
#[inline]
fn timer_counter() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        // CNTVCT: c14
        asm!("mrrc p15, 1, {}, {}, c14", out(reg) low, out(reg) high);
    }
    ((high as u64) << 32) | (low as u64)
}

/// Write CNTP_CVAL register (Counter-timer Physical Timer CompareValue register)
#[inline]
fn write_timer_comparevalue(value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        // CNTP_CVAL: c14, c2
        asm!("mcrr p15, 2, {}, {}, c14", in(reg) low, in(reg) high);
    }
}

/// Write Physical Timer Control register (CNTP_CTL)
#[inline]
fn write_timer_control(control: u32) {
    unsafe {
        // CNTP_CTL: c14, c2
        asm!("mcr p15, 0, {}, c14, c2, 1", in(reg) control);
    }
}

/// Returns the current clock time in hardware ticks.
#[impl_plat_interface]
impl axplat::time::TimeIf for TimeIfImpl {
    fn current_ticks() -> u64 {
        timer_counter()
    }

    /// Converts hardware ticks to nanoseconds.
    fn ticks_to_nanos(ticks: u64) -> u64 {
        let freq = timer_frequency();
        ticks * NANOS_PER_SEC / freq
    }

    /// Converts nanoseconds to hardware ticks.
    fn nanos_to_ticks(nanos: u64) -> u64 {
        let freq = timer_frequency();
        nanos * freq / NANOS_PER_SEC
    }

    fn epochoffset_nanos() -> u64 {
        0
    }

    /// Set a one-shot timer.
    ///
    /// A timer interrupt will be triggered at the specified monotonic time deadline (in nanoseconds).
    #[cfg(feature = "irq")]
    fn set_oneshot_timer(deadline_ns: u64) {
        let current_ns = Self::ticks_to_nanos(Self::current_ticks());
        if deadline_ns > current_ns {
            let ticks = Self::nanos_to_ticks(deadline_ns - current_ns);
            write_timer_comparevalue(Self::current_ticks() + ticks);
            write_timer_control(1); // Enable timer
        } else {
            // Deadline has passed, trigger immediately
            write_timer_comparevalue(Self::current_ticks());
            write_timer_control(1); // Enable timer
        }
    }
}

/// Enable timer interrupts
#[cfg(feature = "irq")]
pub fn enable_irqs(_irq_num: usize) {
    write_timer_comparevalue(timer_counter());
    write_timer_control(1);
}
