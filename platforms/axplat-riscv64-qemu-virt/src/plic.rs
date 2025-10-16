//! PLIC (Platform-Level Interrupt Controller) driver for RISC-V
//!
//! This module implements the PLIC interface for handling external interrupts
//! on RISC-V platforms, specifically for QEMU virt machine.

use core::ptr::{read_volatile, write_volatile};

/// PLIC error types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlicError {
    InvalidInterruptId,
    InvalidContextId,
    InvalidPriority,
    InvalidThreshold,
    NotInitialized,
}

/// PLIC result type
pub type PlicResult<T> = Result<T, PlicError>;

/// PLIC base address from MMIO ranges
const PLIC_BASE: usize = 0x0c00_0000;

/// PLIC register offsets
const PLIC_PRIORITY_BASE: usize = 0x0000;
#[allow(dead_code)]
const PLIC_PENDING_BASE: usize = 0x1000;
const PLIC_ENABLE_BASE: usize = 0x2000;
const PLIC_THRESHOLD_BASE: usize = 0x200000;
const PLIC_CLAIM_BASE: usize = 0x200004;

/// Number of interrupt sources supported by PLIC
const PLIC_MAX_INTERRUPTS: usize = 1024;

/// Number of contexts (hart contexts) supported
const PLIC_MAX_CONTEXTS: usize = 8;

/// PLIC driver structure
pub struct Plic {
    base: usize,
    initialized: bool,
    max_interrupts: usize,
    max_contexts: usize,
}

impl Plic {
    /// Create a new PLIC driver instance
    pub const fn new() -> Self {
        Self { 
            base: PLIC_BASE,
            initialized: false,
            max_interrupts: PLIC_MAX_INTERRUPTS,
            max_contexts: PLIC_MAX_CONTEXTS,
        }
    }

    /// Initialize the PLIC
    pub fn init(&mut self) -> PlicResult<()> {
        info!("Initializing PLIC at base address 0x{:x}", self.base);
        
        // Set priority for all interrupts to 1 (minimum non-zero priority)
        for i in 1..self.max_interrupts {
            if let Err(e) = self.set_priority(i, 1) {
                warn!("Failed to set priority for interrupt {}: {:?}", i, e);
            }
        }
        
        // Set threshold for context 0 (supervisor mode) to 0
        if let Err(e) = self.set_threshold(0, 0) {
            warn!("Failed to set threshold for context 0: {:?}", e);
            return Err(e);
        }
        
        self.initialized = true;
        info!("PLIC initialized successfully");
        Ok(())
    }
    
    /// Check if PLIC is initialized
    #[allow(dead_code)]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    
    /// Validate interrupt ID
    fn validate_interrupt_id(&self, interrupt_id: usize) -> PlicResult<()> {
        if interrupt_id >= self.max_interrupts {
            Err(PlicError::InvalidInterruptId)
        } else {
            Ok(())
        }
    }
    
    /// Validate context ID
    fn validate_context_id(&self, context_id: usize) -> PlicResult<()> {
        if context_id >= self.max_contexts {
            Err(PlicError::InvalidContextId)
        } else {
            Ok(())
        }
    }
    
    /// Validate priority
    fn validate_priority(&self, priority: u32) -> PlicResult<()> {
        if priority > 7 { // PLIC supports priority 0-7
            Err(PlicError::InvalidPriority)
        } else {
            Ok(())
        }
    }
    
    /// Validate threshold
    fn validate_threshold(&self, threshold: u32) -> PlicResult<()> {
        if threshold > 7 { // PLIC supports threshold 0-7
            Err(PlicError::InvalidThreshold)
        } else {
            Ok(())
        }
    }

    /// Set priority for an interrupt source
    pub fn set_priority(&self, interrupt_id: usize, priority: u32) -> PlicResult<()> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_interrupt_id(interrupt_id)?;
        self.validate_priority(priority)?;
        
        let reg_addr = self.base + PLIC_PRIORITY_BASE + (interrupt_id * 4);
        unsafe {
            write_volatile(reg_addr as *mut u32, priority);
        }
        
        Ok(())
    }

    /// Get priority for an interrupt source
    #[allow(dead_code)]
    pub fn get_priority(&self, interrupt_id: usize) -> PlicResult<u32> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_interrupt_id(interrupt_id)?;
        
        let reg_addr = self.base + PLIC_PRIORITY_BASE + (interrupt_id * 4);
        let priority = unsafe {
            read_volatile(reg_addr as *const u32)
        };
        
        Ok(priority)
    }

    /// Enable an interrupt for a specific context
    pub fn enable_interrupt(&self, context_id: usize, interrupt_id: usize) -> PlicResult<()> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        self.validate_interrupt_id(interrupt_id)?;
        
        let reg_addr = self.base + PLIC_ENABLE_BASE + (context_id * 0x80) + ((interrupt_id / 32) * 4);
        let bit = interrupt_id % 32;
        
        unsafe {
            let mut value = read_volatile(reg_addr as *const u32);
            value |= 1 << bit;
            write_volatile(reg_addr as *mut u32, value);
        }
        
        Ok(())
    }

    /// Disable an interrupt for a specific context
    pub fn disable_interrupt(&self, context_id: usize, interrupt_id: usize) -> PlicResult<()> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        self.validate_interrupt_id(interrupt_id)?;
        
        let reg_addr = self.base + PLIC_ENABLE_BASE + (context_id * 0x80) + ((interrupt_id / 32) * 4);
        let bit = interrupt_id % 32;
        
        unsafe {
            let mut value = read_volatile(reg_addr as *const u32);
            value &= !(1 << bit);
            write_volatile(reg_addr as *mut u32, value);
        }
        
        Ok(())
    }

    /// Set threshold for a context
    pub fn set_threshold(&self, context_id: usize, threshold: u32) -> PlicResult<()> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        self.validate_threshold(threshold)?;
        
        let reg_addr = self.base + PLIC_THRESHOLD_BASE + (context_id * 0x1000);
        unsafe {
            write_volatile(reg_addr as *mut u32, threshold);
        }
        
        Ok(())
    }

    /// Get threshold for a context
    #[allow(dead_code)]
    pub fn get_threshold(&self, context_id: usize) -> PlicResult<u32> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        
        let reg_addr = self.base + PLIC_THRESHOLD_BASE + (context_id * 0x1000);
        let threshold = unsafe {
            read_volatile(reg_addr as *const u32)
        };
        
        Ok(threshold)
    }

    /// Claim an interrupt (read the highest priority pending interrupt)
    pub fn claim(&self, context_id: usize) -> PlicResult<Option<usize>> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        
        let reg_addr = self.base + PLIC_CLAIM_BASE + (context_id * 0x1000);
        let interrupt_id = unsafe {
            read_volatile(reg_addr as *const u32)
        };
        
        if interrupt_id == 0 {
            Ok(None)
        } else {
            Ok(Some(interrupt_id as usize))
        }
    }

    /// Complete an interrupt (acknowledge that the interrupt has been handled)
    pub fn complete(&self, context_id: usize, interrupt_id: usize) -> PlicResult<()> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        self.validate_interrupt_id(interrupt_id)?;
        
        let reg_addr = self.base + PLIC_CLAIM_BASE + (context_id * 0x1000);
        unsafe {
            write_volatile(reg_addr as *mut u32, interrupt_id as u32);
        }
        
        Ok(())
    }

    /// Check if an interrupt is pending
    #[allow(dead_code)]
    pub fn is_pending(&self, interrupt_id: usize) -> PlicResult<bool> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_interrupt_id(interrupt_id)?;
        
        let reg_addr = self.base + PLIC_PENDING_BASE + ((interrupt_id / 32) * 4);
        let bit = interrupt_id % 32;
        
        let value = unsafe {
            read_volatile(reg_addr as *const u32)
        };
        
        Ok((value & (1 << bit)) != 0)
    }
    
    /// Batch enable multiple interrupts for a context
    #[allow(dead_code)]
    pub fn enable_interrupts_batch(&self, context_id: usize, interrupt_ids: &[usize]) -> PlicResult<()> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        
        for &interrupt_id in interrupt_ids {
            self.enable_interrupt(context_id, interrupt_id)?;
        }
        
        Ok(())
    }
    
    /// Batch disable multiple interrupts for a context
    #[allow(dead_code)]
    pub fn disable_interrupts_batch(&self, context_id: usize, interrupt_ids: &[usize]) -> PlicResult<()> {
        if !self.initialized {
            return Err(PlicError::NotInitialized);
        }
        
        self.validate_context_id(context_id)?;
        
        for &interrupt_id in interrupt_ids {
            self.disable_interrupt(context_id, interrupt_id)?;
        }
        
        Ok(())
    }
}

/// Global PLIC instance
static mut PLIC: Plic = Plic::new();

/// Initialize the global PLIC instance
pub fn init() -> PlicResult<()> {
    unsafe {
        // SAFETY: We ensure that PLIC is only initialized once
        // and this function is called during system initialization
        let plic_ptr = &raw mut PLIC;
        (*plic_ptr).init()
    }
}

/// Get a reference to the global PLIC instance
pub fn get() -> &'static Plic {
    unsafe {
        // SAFETY: We ensure that PLIC is only modified during init
        // and this function is called after init, so it's safe to create a reference
        // We use a raw pointer to avoid the static_mut_refs warning
        #[allow(clippy::deref_addrof)]
        &*&raw const PLIC
    }
}
