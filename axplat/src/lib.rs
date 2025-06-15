#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

#[macro_use]
extern crate axplat_macros;

pub mod console;
pub mod init;
pub mod irq;
pub mod mem;
pub mod power;
pub mod time;

pub use axplat_macros::{main, secondary_main};
pub use crate_interface::impl_interface as impl_plat_interface;

#[doc(hidden)]
pub mod __priv {
    pub use crate_interface::{call_interface, def_interface};
}

/// Asserts that constant expressions evaluate to `true`.
///
/// Constant expressions can be ensured to have certain properties via this
/// macro. If the expression evaluates to `false`, the file will fail to compile.
///
/// # Examples
///
/// A common use case is to guarantee properties about a constant value that's
/// generated via meta-programming.
///
/// ```
/// #[macro_use]
/// extern crate axplat;
/// const VALUE: i32 = 3;
/// const_assert!(VALUE >= 2);
/// fn main() {}
/// ```
///
/// The following fails to compile because multiplying by 5 does not have an
/// identity property:
///
/// ```compile_fail
/// #[macro_use]
/// extern crate axplat;
/// const_assert!(5 * 5 == 5);
/// fn main() {}
/// ```
///
/// This macro is a reference to [static_assert](https://crates.io/crates/static_assertions).
#[macro_export]
macro_rules! const_assert {
    ($x:expr $(,)?) => {
        #[allow(unknown_lints, eq_op)]
        const _: [(); 0 - !{
            const ASSERT: bool = $x;
            ASSERT
        } as usize] = [];
    };
}

/// Call the function decorated by [`axplat::main`][main] for the primary core.
///
/// This function should only be called by the platform implementer, not the kernel.
pub fn call_main(cpu_id: usize, arg: usize) -> ! {
    unsafe { __axplat_main(cpu_id, arg) }
}

/// Call the function decorated by [`axplat::secondary_main`][secondary_main] for secondary cores.
///
/// This function should only be called by the platform implementer, not the kernel.
pub fn call_secondary_main(cpu_id: usize) -> ! {
    unsafe { __axplat_secondary_main(cpu_id) }
}

unsafe extern "Rust" {
    fn __axplat_main(cpu_id: usize, arg: usize) -> !;
    fn __axplat_secondary_main(cpu_id: usize) -> !;
}
