use axplat::power::PowerIf;

struct PowerImpl;

#[impl_plat_interface]
impl PowerIf for PowerImpl {
    /// Bootstraps the given CPU core with the given initial stack (in physical
    /// address).
    ///
    /// Where `cpu_id` is the logical CPU ID (0, 1, ..., N-1, N is the number of
    /// CPU cores on the platform).
    fn cpu_boot(cpu_id: usize, stack_top_paddr: usize) {
        #[cfg(feature = "smp")]
        {
            let entry_paddr =
                axplat::mem::virt_to_phys(va!(crate::boot::_start_secondary as usize));
            axplat_aarch64_common::psci::cpu_on(cpu_id, entry_paddr.as_usize(), stack_top_paddr);
        }
        #[cfg(not(feature = "smp"))]
        {
            let _ = (cpu_id, stack_top_paddr);
            log::warn!(
                "feature `smp` is not enabled for crate `{}`!",
                env!("CARGO_CRATE_NAME")
            );
        }
    }

    /// Shutdown the whole system.
    fn system_off() -> ! {
        axplat_aarch64_common::psci::system_off()
    }
}
