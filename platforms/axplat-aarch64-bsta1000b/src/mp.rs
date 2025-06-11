use crate::config::devices::CPU_ID_LIST;
use crate::mem::virt_to_phys;
use memory_addr::PhysAddr;

/// Hart number of bsta1000b board
pub const MAX_HARTS: usize = 8;

/// Starts the given secondary CPU with its boot stack.
pub fn start_secondary_cpu(cpu_id: usize, stack_top: PhysAddr) {
    if cpu_id >= MAX_HARTS {
        error!("No support for bsta1000b core {}", cpu_id);
        return;
    }

    let entry = virt_to_phys(va!(crate::boot::_start_secondary as usize));
    axplat_aarch64_common::psci::cpu_on(
        CPU_ID_LIST[cpu_id],
        entry.as_usize(),
        stack_top.as_usize(),
    );
}
