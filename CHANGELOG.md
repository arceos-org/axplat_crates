# Changelog

## 0.4.x

### New Features

- Add IPI support for LoongArch platform (https://github.com/arceos-org/axplat_crates/pull/24).
- Add new platform `axplat-arm-qemu-virt` for QEMU ARM virtual machine (https://github.com/arceos-org/axplat_crates/pull/32).

### Breaking Changes

- Upgrade [crate_interface](https://crates.io/crates/crate_interface) dependency to v0.3, and replace `axplat_macros::def_plat_interface` with `crate_interface::def_interface(gen_caller)` (https://github.com/arceos-org/axplat_crates/pull/46).
- Upgrade [axcpu](https://crates.io/crates/axcpu) dependency to v0.3 and [page_table_multiarch](https://crates.io/crates/page_table_multiarch) dependency to v0.6 for ARM support.
- Upgrade [percpu](https://crates.io/crates/percpu) dependency to v0.3, see [percpu v0.3.0 changelog](https://github.com/arceos-org/percpu/blob/main/CHANGELOG.md#030).
- Rename crate `axplat-aarch64-peripherals` to `axplat-arm-peripherals` (https://github.com/arceos-org/axplat_crates/pull/44).

## 0.3.x

### Breaking Changes

- Add `cpu_num` method to get CPU count dynamically (https://github.com/arceos-org/axplat_crates/pull/33).

### New Features

- Add PSCI support for ARM architecture in SMC and HVC calls (https://github.com/arceos-org/axplat_crates/pull/31).

### Bug Fixes

- Fix IPI delivery on x86-pc by mapping cpu_id to apic_id (https://github.com/arceos-org/axplat_crates/pull/30).
- Clear RISC-V software interrupt after handling IPI (https://github.com/arceos-org/axplat_crates/pull/28).
- Fix linker script to keep only `_start` in the `.text` section (https://github.com/arceos-org/axplat_crates/pull/26).

## 0.2.x

### New Features

- Enrich IPI related APIs (https://github.com/arceos-org/axplat_crates/pull/21).

### Bug Fixes

- Add `.code64` directive for 64-bit code section (https://github.com/arceos-org/axplat_crates/pull/22).
- Fix RISC-V physical RAM range calculation (https://github.com/arceos-org/axplat_crates/pull/19).

### Internal Changes

- Add description and keywords, fix documentation links for platform crates.
- Add targets for docs.rs build.
- Upgrade [x2apic](https://crates.io/crates/x2apic) dependency to v0.5 (https://github.com/arceos-org/axplat_crates/pull/17).

## 0.1.0

### Initial Features

- Initial release with core platform abstraction layer functionality.
- Support for multiple architectures: x86_64, aarch64, riscv64, loongarch64.
- Platform abstraction traits for console, IRQ, memory, power, and time.
- Basic support for x86-pc, QEMU-based virt platforms (aarch64, riscv64, loongarch64), and ARM64 SoCs (Raspberry Pi 4, Phytium Pi, BST A1000).
- CLI tool `cargo-axplat` for creating and managing platform packages.
- Minimal example kernels: [hello-kernel](examples/hello-kernel), [irq-kernel](examples/irq-kernel), [smp-kernel](examples/smp-kernel).

# Changelog for [cargo-axplat](cargo-axplat)

## 0.3.0

- Rename `axplat::impl_plat_interface` to `axplat::impl_interface` (https://github.com/arceos-org/axplat_crates/pull/46).
- Support UTF-8 paths for `cargo-axplat` (https://github.com/arceos-org/axplat_crates/pull/48).

## 0.2.5

- Add `--axplat-path` argument to force a generated template to depend on a local axplat path (https://github.com/arceos-org/axplat_crates/pull/40).

## 0.2.4

- Add support for `cpu_num` configuration in platform packages (https://github.com/arceos-org/axplat_crates/pull/33).

## 0.2.2

- Show stderr of cargo metadata.
- Add `-C` argument to support out-of-tree usage.

## 0.2.1

- Fix `axplat` version.

## 0.2.0

- Initial release of `cargo-axplat` CLI tool for creating and managing `axplat` platform packages.
