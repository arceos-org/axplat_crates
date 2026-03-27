use axplat::mem::{pa, phys_to_virt};

use crate::config::plat::PSCI_METHOD;

#[cfg(feature = "irq")]
const TIMER_IRQ: usize = crate::config::devices::TIMER_IRQ;

struct InitIfImpl;

#[impl_interface]
impl axplat::init::InitIf for InitIfImpl {
    /// Initializes the platform at the early stage for the primary core.
    ///
    /// This function should be called immediately after the kernel has booted,
    /// and performed earliest platform configuration and initialization (e.g.,
    /// early console, clocking).
    fn init_early(_cpu_id: usize, _dtb: usize) {
        axcpu::init::init_trap();
        axplat_arm_peripherals::pl011::init_early(phys_to_virt(pa!(
            crate::config::devices::UART_PADDR
        )));
        axplat_arm_peripherals::psci::init(PSCI_METHOD);
        axplat_arm_peripherals::generic_timer::init_early();

        axplat::console_println!("init_early on QEMU VIRT platform");
        #[cfg(feature = "rtc")]
        axplat_arm_peripherals::pl031::init_early(phys_to_virt(pa!(
            crate::config::devices::RTC_PADDR
        )));
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
            axplat_arm_peripherals::gic::init_gic(
                phys_to_virt(pa!(crate::config::devices::GICD_PADDR)),
                phys_to_virt(pa!(crate::config::devices::GICC_PADDR)),
            );
            axplat_arm_peripherals::gic::init_gicc();
            axplat_arm_peripherals::generic_timer::enable_irqs(TIMER_IRQ);
        }
    }

    /// Initializes the platform at the later stage for secondary cores.
    #[cfg(feature = "smp")]
    fn init_later_secondary(_cpu_id: usize) {
        #[cfg(feature = "irq")]
        {
            axplat_arm_peripherals::gic::init_gicc();
            axplat_arm_peripherals::generic_timer::enable_irqs(TIMER_IRQ);
        }
    }
}
