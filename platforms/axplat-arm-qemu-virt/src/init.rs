use axplat::mem::phys_to_virt;
use memory_addr::pa;

use crate::config::plat::PSCI_METHOD;

#[cfg(feature = "irq")]
const TIMER_IRQ: usize = crate::config::devices::TIMER_IRQ;

struct InitIfImpl;

#[impl_plat_interface]
impl axplat::init::InitIf for InitIfImpl {
    /// Initializes the platform at the early stage for the primary core.
    ///
    /// This function should be called immediately after the kernel has booted,
    /// and performed earliest platform configuration and initialization (e.g.,
    /// early console, clocking).
    fn init_early(_cpu_id: usize, _dtb: usize) {
        axcpu::init::init_trap();
        axplat_aarch64_peripherals::pl011::init_early(phys_to_virt(pa!(
            crate::config::devices::UART_PADDR
        )));
        axplat_aarch64_peripherals::psci::init(PSCI_METHOD);

        axplat::console_println!("init_early on QEMU VIRT platform");
        #[cfg(feature = "rtc")]
        axplat_aarch64_peripherals::pl031::init_early(phys_to_virt(pa!(RTC_PADDR)));
    }

    /// Initializes the platform at the early stage for secondary cores.
    #[cfg(feature = "smp")]
    fn init_early_secondary(_cpu_id: usize) {
        axcpu::init::init_trap();
    }

    /// Initializes the platform at the later stage for the primary core.
    ///
    /// This function should be called after the kernel has done part of its
    /// initialization (e.g, logging, memory management), and finalized the rest of
    /// platform configuration and initialization.
    fn init_later(_cpu_id: usize, _dtb: usize) {
        #[cfg(feature = "irq")]
        {
            axplat_aarch64_peripherals::gic::init_gic(
                phys_to_virt(pa!(crate::config::devices::GICD_PADDR)),
                phys_to_virt(pa!(crate::config::devices::GICC_PADDR)),
            );
            axplat_aarch64_peripherals::gic::init_gicc();
            crate::generic_timer::enable_irqs(TIMER_IRQ);
        }
    }

    /// Initializes the platform at the later stage for secondary cores.
    #[cfg(feature = "smp")]
    fn init_later_secondary(_cpu_id: usize) {
        #[cfg(feature = "irq")]
        {
            crate::irq::init_current_cpu();
            crate::generic_timer::enable_irqs(TIMER_IRQ);
        }
    }
}
