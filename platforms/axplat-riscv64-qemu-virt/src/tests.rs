//! Test module for RISC-V platform implementation
//!
//! This module contains functionality tests for the RISC-V platform
//! hardware abstraction layer implementation.

/// Test utilities for RISC-V platform
pub mod test_utils {
    use crate::dtb::MemoryRange;

    /// Test memory range structure
    pub fn test_memory_range() -> bool {
        let range = MemoryRange {
            base: 0x80000000,
            size: 0x8000000,
        };
        
        // Test basic properties
        if range.base != 0x80000000 || range.size != 0x8000000 {
            return false;
        }
        
        // Test clone and copy
        let range2 = range;
        if range.base != range2.base || range.size != range2.size {
            return false;
        }
        
        let range3 = range.clone();
        if range.base != range3.base || range.size != range3.size {
            return false;
        }
        
        true
    }

    /// Test basic constants and configurations
    pub fn test_basic_constants() -> bool {
        // Test that basic constants are defined correctly
        crate::config::plat::PHYS_MEMORY_BASE > 0
            && crate::config::plat::PHYS_MEMORY_SIZE > 0
            && crate::config::plat::KERNEL_BASE_PADDR > 0
            && crate::config::plat::PHYS_VIRT_OFFSET > 0
    }

    /// Test device tree parser basic functionality
    pub fn test_dtb_parser_basic() -> bool {
        use crate::dtb::DtbParser;
        
        // Test with null pointer
        unsafe {
            if let Ok(_) = DtbParser::new(core::ptr::null()) {
                return false; // Should fail
            }
        }
        
        // Test with invalid magic number
        let mut invalid_dtb = [0u8; 64];
        unsafe {
            if let Ok(_) = DtbParser::new(invalid_dtb.as_ptr()) {
                return false; // Should fail
            }
        }
        
        true
    }

    /// Test device tree parser information methods
    pub fn test_dtb_parser_info() -> bool {
        use crate::dtb::DtbParser;
        
        // Create a mock parser for testing
        let parser = DtbParser {
            dtb_ptr: core::ptr::null(),
            struct_ptr: core::ptr::null(),
            strings_ptr: core::ptr::null(),
            valid: false,
        };
        
        // Test with invalid parser
        if parser.get_memory_ranges().len() != 0 {
            return false;
        }
        if parser.get_cpu_count() != 1 {
            return false; // Default fallback
        }
        if parser.get_timer_frequency().is_some() {
            return false;
        }
        if parser.get_uart_base().is_some() {
            return false;
        }
        if parser.get_plic_base().is_some() {
            return false;
        }
        
        // Test with valid parser (should return defaults)
        let parser_valid = DtbParser {
            dtb_ptr: core::ptr::null(),
            struct_ptr: core::ptr::null(),
            strings_ptr: core::ptr::null(),
            valid: true,
        };
        
        let memory_ranges = parser_valid.get_memory_ranges();
        if memory_ranges.len() != 1 {
            return false;
        }
        if memory_ranges[0].base != 0x80000000 || memory_ranges[0].size != 0x8000000 {
            return false;
        }
        
        if parser_valid.get_cpu_count() != 1 {
            return false;
        }
        if parser_valid.get_timer_frequency() != Some(10_000_000) {
            return false;
        }
        if parser_valid.get_uart_base() != Some(0x10000000) {
            return false;
        }
        if parser_valid.get_plic_base() != Some(0x0c000000) {
            return false;
        }
        
        true
    }
}

/// PLIC driver tests (only when irq feature is enabled)
#[cfg(feature = "irq")]
pub mod plic_test_utils {
    use crate::plic::{Plic, PlicError};

    /// Test PLIC driver basic functionality
    pub fn test_plic_basic_functionality() -> bool {
        // Test PLIC creation
        let plic = Plic::new();
        if plic.is_initialized() {
            return false;
        }
        
        // Test validation through public APIs
        if plic.set_priority(0, 1) != Err(PlicError::InvalidInterruptId) {
            return false;
        }
        if plic.set_priority(1, 1) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.set_priority(1024, 1) != Err(PlicError::InvalidInterruptId) {
            return false;
        }
        
        if plic.set_priority(1, 8) != Err(PlicError::InvalidPriority) {
            return false;
        }
        if plic.set_threshold(8, 0) != Err(PlicError::InvalidContextId) {
            return false;
        }
        if plic.set_threshold(0, 8) != Err(PlicError::InvalidThreshold) {
            return false;
        }
        
        true
    }

    /// Test PLIC error handling
    pub fn test_plic_error_handling() -> bool {
        let plic = Plic::new();
        
        // Test operations on uninitialized PLIC
        if plic.set_priority(1, 1) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.get_priority(1) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.enable_interrupt(0, 1) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.disable_interrupt(0, 1) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.set_threshold(0, 0) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.get_threshold(0) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.claim(0) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.complete(0, 1) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.is_pending(1) != Err(PlicError::NotInitialized) {
            return false;
        }
        
        true
    }

    /// Test PLIC batch operations
    pub fn test_plic_batch_operations() -> bool {
        let plic = Plic::new();
        
        // Test batch operations on uninitialized PLIC
        if plic.enable_interrupts_batch(0, &[1, 2, 3]) != Err(PlicError::NotInitialized) {
            return false;
        }
        if plic.disable_interrupts_batch(0, &[1, 2, 3]) != Err(PlicError::NotInitialized) {
            return false;
        }
        
        // Test with invalid context
        if plic.enable_interrupts_batch(8, &[1, 2, 3]) != Err(PlicError::InvalidContextId) {
            return false;
        }
        if plic.disable_interrupts_batch(8, &[1, 2, 3]) != Err(PlicError::InvalidContextId) {
            return false;
        }
        
        true
    }
}

/// Run all tests
pub fn run_all_tests() -> bool {
    use crate::test_utils::*;
    
    let mut all_passed = true;
    
    // Run basic tests
    if !test_memory_range() {
        crate::log::error!("Memory range test failed");
        all_passed = false;
    }
    
    if !test_basic_constants() {
        crate::log::error!("Basic constants test failed");
        all_passed = false;
    }
    
    if !test_dtb_parser_basic() {
        crate::log::error!("DTB parser basic test failed");
        all_passed = false;
    }
    
    if !test_dtb_parser_info() {
        crate::log::error!("DTB parser info test failed");
        all_passed = false;
    }
    
    // Run PLIC tests if irq feature is enabled
    #[cfg(feature = "irq")]
    {
        use crate::plic_test_utils::*;
        
        if !test_plic_basic_functionality() {
            crate::log::error!("PLIC basic functionality test failed");
            all_passed = false;
        }
        
        if !test_plic_error_handling() {
            crate::log::error!("PLIC error handling test failed");
            all_passed = false;
        }
        
        if !test_plic_batch_operations() {
            crate::log::error!("PLIC batch operations test failed");
            all_passed = false;
        }
    }
    
    if all_passed {
        crate::log::info!("All tests passed!");
    } else {
        crate::log::error!("Some tests failed!");
    }
    
    all_passed
}
