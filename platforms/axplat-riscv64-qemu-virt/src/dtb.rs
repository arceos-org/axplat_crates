//! Device Tree Blob (DTB) parsing utilities for RISC-V
//!
//! This module provides functionality to parse the device tree blob
//! passed by the bootloader to extract hardware information.

extern crate alloc;

use core::ptr;
use alloc::vec::Vec;

/// Device Tree Blob header structure
#[repr(C)]
struct DtbHeader {
    magic: u32,
    totalsize: u32,
    off_dt_struct: u32,
    off_dt_strings: u32,
    off_mem_rsvmap: u32,
    version: u32,
    last_comp_version: u32,
    boot_cpuid_phys: u32,
    size_dt_strings: u32,
    size_dt_struct: u32,
}

/// Device Tree token types
const FDT_BEGIN_NODE: u32 = 0x00000001;
const FDT_END_NODE: u32 = 0x00000002;
const FDT_PROP: u32 = 0x00000003;
const FDT_NOP: u32 = 0x00000004;
const FDT_END: u32 = 0x00000009;

/// Device Tree property structure
#[repr(C)]
struct DtbProperty {
    len: u32,
    nameoff: u32,
}

/// Memory range information from device tree
#[derive(Debug, Clone, Copy)]
pub struct MemoryRange {
    pub base: u64,
    pub size: u64,
}

/// Device tree parser
pub struct DtbParser {
    dtb_ptr: *const u8,
    struct_ptr: *const u32,
    strings_ptr: *const u8,
    valid: bool,
}

impl DtbParser {
    /// Create a new DTB parser from a raw DTB pointer
    pub unsafe fn new(dtb_ptr: *const u8) -> Result<Self, &'static str> {
        if dtb_ptr.is_null() {
            return Err("DTB pointer is null");
        }

        let header = dtb_ptr as *const DtbHeader;
        
        // Validate DTB magic number
        let magic = unsafe { ptr::read_volatile(&(*header).magic) };
        if magic != 0xd00dfeed {
            return Err("Invalid DTB magic number");
        }
        
        // Get header information
        let _totalsize = unsafe { ptr::read_volatile(&(*header).totalsize) };
        let off_dt_struct = unsafe { ptr::read_volatile(&(*header).off_dt_struct) };
        let off_dt_strings = unsafe { ptr::read_volatile(&(*header).off_dt_strings) };
        
        // Calculate pointers
        let struct_ptr = unsafe { dtb_ptr.add(off_dt_struct as usize) as *const u32 };
        let strings_ptr = unsafe { dtb_ptr.add(off_dt_strings as usize) };
        
        Ok(Self {
            dtb_ptr,
            struct_ptr,
            strings_ptr,
            valid: true,
        })
    }

    /// Get memory ranges from the device tree
    pub fn get_memory_ranges(&self) -> Vec<MemoryRange> {
        if !self.valid {
            return Vec::new();
        }
        
        let mut ranges = Vec::new();
        self.parse_memory_nodes(&mut ranges);
        
        if ranges.is_empty() {
            // Fallback to default range for QEMU virt machine
            ranges.push(MemoryRange {
                base: 0x80000000,
                size: 0x8000000, // 128MB
            });
        }
        
        ranges
    }
    
    /// Parse memory nodes from device tree
    fn parse_memory_nodes(&self, ranges: &mut Vec<MemoryRange>) {
        unsafe {
            let mut current = self.struct_ptr;
            let mut depth = 0;
            let mut in_memory_node = false;
            
            loop {
                let token = ptr::read_volatile(current);
                current = current.add(1);
                
                match token {
                    FDT_BEGIN_NODE => {
                        depth += 1;
                        if depth == 1 {
                            // Check if this is a memory node
                            let node_name = current as *const u8;
                            let name_len = self.strlen(node_name);
                            if name_len >= 6 && self.strncmp(node_name, c"memory".as_ptr(), 6) == 0 {
                                in_memory_node = true;
                            }
                        }
                        // Skip node name (null-terminated string)
                        let name_len = self.strlen(current as *const u8);
                        current = current.add((name_len + 4) / 4); // Align to 4-byte boundary
                    }
                    FDT_END_NODE => {
                        depth -= 1;
                        if depth == 0 {
                            in_memory_node = false;
                        }
                        if depth < 0 {
                            break;
                        }
                    }
                    FDT_PROP => {
                        if in_memory_node {
                            self.parse_memory_property(current, ranges);
                        }
                        // Skip property
                        let prop = current as *const DtbProperty;
                        let prop_len = ptr::read_volatile(&(*prop).len);
                        current = current.add(2); // Skip property header
                        current = current.add((prop_len as usize).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_NOP => {
                        // Do nothing
                    }
                    FDT_END => {
                        break;
                    }
                    _ => {
                        // Unknown token, skip
                        break;
                    }
                }
            }
        }
    }
    
    /// Parse memory property (reg property)
    fn parse_memory_property(&self, prop_ptr: *const u32, ranges: &mut Vec<MemoryRange>) {
        unsafe {
            let prop = prop_ptr as *const DtbProperty;
            let prop_len = ptr::read_volatile(&(*prop).len);
            let nameoff = ptr::read_volatile(&(*prop).nameoff);
            
            // Get property name
            let name_ptr = self.strings_ptr.add(nameoff as usize);
            if self.strncmp(name_ptr, c"reg".as_ptr(), 3) != 0 {
                return; // Not a reg property
            }
            
            // Parse reg property (address, size pairs)
            let data_ptr = prop_ptr.add(2) as *const u8;
            let mut offset = 0;
            
            while offset < prop_len as usize {
                if offset + 16 > prop_len as usize {
                    break; // Not enough data for address+size pair
                }
                
                // Read 64-bit address and size (big-endian)
                let addr_high = u32::from_be_bytes([
                    *data_ptr.add(offset),
                    *data_ptr.add(offset + 1),
                    *data_ptr.add(offset + 2),
                    *data_ptr.add(offset + 3),
                ]);
                let addr_low = u32::from_be_bytes([
                    *data_ptr.add(offset + 4),
                    *data_ptr.add(offset + 5),
                    *data_ptr.add(offset + 6),
                    *data_ptr.add(offset + 7),
                ]);
                let size_high = u32::from_be_bytes([
                    *data_ptr.add(offset + 8),
                    *data_ptr.add(offset + 9),
                    *data_ptr.add(offset + 10),
                    *data_ptr.add(offset + 11),
                ]);
                let size_low = u32::from_be_bytes([
                    *data_ptr.add(offset + 12),
                    *data_ptr.add(offset + 13),
                    *data_ptr.add(offset + 14),
                    *data_ptr.add(offset + 15),
                ]);
                
                let base = ((addr_high as u64) << 32) | (addr_low as u64);
                let size = ((size_high as u64) << 32) | (size_low as u64);
                
                if size > 0 {
                    ranges.push(MemoryRange { base, size });
                }
                
                offset += 16;
            }
        }
    }
    
    /// Helper function to get string length
    unsafe fn strlen(&self, s: *const u8) -> usize {
        let mut len = 0;
        while unsafe { *s.add(len) } != 0 {
            len += 1;
        }
        len
    }
    
    /// Helper function to compare strings
    unsafe fn strncmp(&self, s1: *const u8, s2: *const u8, n: usize) -> i32 {
        for i in 0..n {
            let c1 = unsafe { *s1.add(i) };
            let c2 = unsafe { *s2.add(i) };
            if c1 != c2 {
                return c1 as i32 - c2 as i32;
            }
            if c1 == 0 {
                break;
            }
        }
        0
    }

    /// Get CPU count from device tree
    pub fn get_cpu_count(&self) -> usize {
        if !self.valid {
            return 1;
        }
        
        let mut cpu_count = 0;
        self.parse_cpu_nodes(&mut cpu_count);
        
        if cpu_count == 0 {
            1 // Default to 1 CPU if parsing fails
        } else {
            cpu_count
        }
    }
    
    /// Parse CPU nodes from device tree
    fn parse_cpu_nodes(&self, cpu_count: &mut usize) {
        unsafe {
            let mut current = self.struct_ptr;
            let mut depth = 0;
            
            loop {
                let token = ptr::read_volatile(current);
                current = current.add(1);
                
                match token {
                    FDT_BEGIN_NODE => {
                        depth += 1;
                        if depth == 1 {
                            // Check if this is a CPU node
                            let node_name = current as *const u8;
                            let name_len = self.strlen(node_name);
                            if name_len >= 3 && self.strncmp(node_name, c"cpu".as_ptr(), 3) == 0 {
                                *cpu_count += 1;
                            }
                        }
                        // Skip node name (null-terminated string)
                        let name_len = self.strlen(current as *const u8);
                        current = current.add((name_len + 4) / 4); // Align to 4-byte boundary
                    }
                    FDT_END_NODE => {
                        depth -= 1;
                        if depth < 0 {
                            break;
                        }
                    }
                    FDT_PROP => {
                        // Skip property
                        let prop = current as *const DtbProperty;
                        let prop_len = ptr::read_volatile(&(*prop).len);
                        current = current.add(2); // Skip property header
                        current = current.add((prop_len as usize).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_NOP => {
                        // Do nothing
                    }
                    FDT_END => {
                        break;
                    }
                    _ => {
                        // Unknown token, skip
                        break;
                    }
                }
            }
        }
    }

    /// Get timer frequency from device tree
    pub fn get_timer_frequency(&self) -> Option<u64> {
        if !self.valid {
            return None;
        }
        
        // Try to parse timer frequency from device tree
        let mut frequency = None;
        self.parse_timer_nodes(&mut frequency);
        
        // Fallback to default timer frequency for QEMU virt machine
        frequency.or(Some(10_000_000)) // 10 MHz
    }
    
    /// Parse timer nodes from device tree
    fn parse_timer_nodes(&self, frequency: &mut Option<u64>) {
        unsafe {
            let mut current = self.struct_ptr;
            let mut depth = 0;
            let mut in_timer_node = false;
            
            loop {
                let token = ptr::read_volatile(current);
                current = current.add(1);
                
                match token {
                    FDT_BEGIN_NODE => {
                        depth += 1;
                        if depth == 1 {
                            // Check if this is a timer node
                            let node_name = current as *const u8;
                            let name_len = self.strlen(node_name);
                            if name_len >= 5 && self.strncmp(node_name, c"timer".as_ptr(), 5) == 0 {
                                in_timer_node = true;
                            }
                        }
                        // Skip node name (null-terminated string)
                        let name_len = self.strlen(current as *const u8);
                        current = current.add((name_len + 4).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_END_NODE => {
                        depth -= 1;
                        if depth == 0 {
                            in_timer_node = false;
                        }
                        if depth < 0 {
                            break;
                        }
                    }
                    FDT_PROP => {
                        if in_timer_node {
                            self.parse_timer_property(current, frequency);
                        }
                        // Skip property
                        let prop = current as *const DtbProperty;
                        let prop_len = ptr::read_volatile(&(*prop).len);
                        current = current.add(2); // Skip property header
                        current = current.add((prop_len as usize).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_NOP => {
                        // Do nothing
                    }
                    FDT_END => {
                        break;
                    }
                    _ => {
                        // Unknown token, skip
                        break;
                    }
                }
            }
        }
    }
    
    /// Parse timer property (clock-frequency)
    fn parse_timer_property(&self, prop_ptr: *const u32, frequency: &mut Option<u64>) {
        unsafe {
            let prop = prop_ptr as *const DtbProperty;
            let prop_len = ptr::read_volatile(&(*prop).len);
            let nameoff = ptr::read_volatile(&(*prop).nameoff);
            
            // Get property name
            let name_ptr = self.strings_ptr.add(nameoff as usize);
            if self.strncmp(name_ptr, c"clock-frequency".as_ptr(), 14) != 0 {
                return; // Not a clock-frequency property
            }
            
            // Parse clock-frequency property (32-bit value)
            if prop_len >= 4 {
                let data_ptr = prop_ptr.add(2) as *const u8;
                let freq = u32::from_be_bytes([
                    *data_ptr,
                    *data_ptr.add(1),
                    *data_ptr.add(2),
                    *data_ptr.add(3),
                ]);
                *frequency = Some(freq as u64);
            }
        }
    }

    /// Get UART base address from device tree
    pub fn get_uart_base(&self) -> Option<u64> {
        if !self.valid {
            return None;
        }
        
        // Try to parse UART base from device tree
        let mut uart_base = None;
        self.parse_uart_nodes(&mut uart_base);
        
        // Fallback to default UART base for QEMU virt machine
        uart_base.or(Some(0x10000000))
    }
    
    /// Parse UART nodes from device tree
    fn parse_uart_nodes(&self, uart_base: &mut Option<u64>) {
        unsafe {
            let mut current = self.struct_ptr;
            let mut depth = 0;
            let mut in_uart_node = false;
            
            loop {
                let token = ptr::read_volatile(current);
                current = current.add(1);
                
                match token {
                    FDT_BEGIN_NODE => {
                        depth += 1;
                        if depth == 1 {
                            // Check if this is a UART node
                            let node_name = current as *const u8;
                            let name_len = self.strlen(node_name);
                            if name_len >= 4 && self.strncmp(node_name, c"uart".as_ptr(), 4) == 0 {
                                in_uart_node = true;
                            }
                        }
                        // Skip node name (null-terminated string)
                        let name_len = self.strlen(current as *const u8);
                        current = current.add((name_len + 4).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_END_NODE => {
                        depth -= 1;
                        if depth == 0 {
                            in_uart_node = false;
                        }
                        if depth < 0 {
                            break;
                        }
                    }
                    FDT_PROP => {
                        if in_uart_node {
                            self.parse_uart_property(current, uart_base);
                        }
                        // Skip property
                        let prop = current as *const DtbProperty;
                        let prop_len = ptr::read_volatile(&(*prop).len);
                        current = current.add(2); // Skip property header
                        current = current.add((prop_len as usize).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_NOP => {
                        // Do nothing
                    }
                    FDT_END => {
                        break;
                    }
                    _ => {
                        // Unknown token, skip
                        break;
                    }
                }
            }
        }
    }
    
    /// Parse UART property (reg)
    fn parse_uart_property(&self, prop_ptr: *const u32, uart_base: &mut Option<u64>) {
        unsafe {
            let prop = prop_ptr as *const DtbProperty;
            let prop_len = ptr::read_volatile(&(*prop).len);
            let nameoff = ptr::read_volatile(&(*prop).nameoff);
            
            // Get property name
            let name_ptr = self.strings_ptr.add(nameoff as usize);
            if self.strncmp(name_ptr, c"reg".as_ptr(), 3) != 0 {
                return; // Not a reg property
            }
            
            // Parse reg property (address, size pairs)
            if prop_len >= 8 {
                let data_ptr = prop_ptr.add(2) as *const u8;
                // Read 64-bit address (big-endian)
                let addr_high = u32::from_be_bytes([
                    *data_ptr,
                    *data_ptr.add(1),
                    *data_ptr.add(2),
                    *data_ptr.add(3),
                ]);
                let addr_low = u32::from_be_bytes([
                    *data_ptr.add(4),
                    *data_ptr.add(5),
                    *data_ptr.add(6),
                    *data_ptr.add(7),
                ]);
                
                let base = ((addr_high as u64) << 32) | (addr_low as u64);
                *uart_base = Some(base);
            }
        }
    }

    /// Get PLIC base address from device tree
    pub fn get_plic_base(&self) -> Option<u64> {
        if !self.valid {
            return None;
        }
        
        // Try to parse PLIC base from device tree
        let mut plic_base = None;
        self.parse_plic_nodes(&mut plic_base);
        
        // Fallback to default PLIC base for QEMU virt machine
        plic_base.or(Some(0x0c000000))
    }
    
    /// Parse PLIC nodes from device tree
    fn parse_plic_nodes(&self, plic_base: &mut Option<u64>) {
        unsafe {
            let mut current = self.struct_ptr;
            let mut depth = 0;
            let mut in_plic_node = false;
            
            loop {
                let token = ptr::read_volatile(current);
                current = current.add(1);
                
                match token {
                    FDT_BEGIN_NODE => {
                        depth += 1;
                        if depth == 1 {
                            // Check if this is a PLIC node
                            let node_name = current as *const u8;
                            let name_len = self.strlen(node_name);
                            if name_len >= 4 && self.strncmp(node_name, c"plic".as_ptr(), 4) == 0 {
                                in_plic_node = true;
                            }
                        }
                        // Skip node name (null-terminated string)
                        let name_len = self.strlen(current as *const u8);
                        current = current.add((name_len + 4).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_END_NODE => {
                        depth -= 1;
                        if depth == 0 {
                            in_plic_node = false;
                        }
                        if depth < 0 {
                            break;
                        }
                    }
                    FDT_PROP => {
                        if in_plic_node {
                            self.parse_plic_property(current, plic_base);
                        }
                        // Skip property
                        let prop = current as *const DtbProperty;
                        let prop_len = ptr::read_volatile(&(*prop).len);
                        current = current.add(2); // Skip property header
                        current = current.add((prop_len as usize).div_ceil(4)); // Align to 4-byte boundary
                    }
                    FDT_NOP => {
                        // Do nothing
                    }
                    FDT_END => {
                        break;
                    }
                    _ => {
                        // Unknown token, skip
                        break;
                    }
                }
            }
        }
    }
    
    /// Parse PLIC property (reg)
    fn parse_plic_property(&self, prop_ptr: *const u32, plic_base: &mut Option<u64>) {
        unsafe {
            let prop = prop_ptr as *const DtbProperty;
            let prop_len = ptr::read_volatile(&(*prop).len);
            let nameoff = ptr::read_volatile(&(*prop).nameoff);
            
            // Get property name
            let name_ptr = self.strings_ptr.add(nameoff as usize);
            if self.strncmp(name_ptr, c"reg".as_ptr(), 3) != 0 {
                return; // Not a reg property
            }
            
            // Parse reg property (address, size pairs)
            if prop_len >= 8 {
                let data_ptr = prop_ptr.add(2) as *const u8;
                // Read 64-bit address (big-endian)
                let addr_high = u32::from_be_bytes([
                    *data_ptr,
                    *data_ptr.add(1),
                    *data_ptr.add(2),
                    *data_ptr.add(3),
                ]);
                let addr_low = u32::from_be_bytes([
                    *data_ptr.add(4),
                    *data_ptr.add(5),
                    *data_ptr.add(6),
                    *data_ptr.add(7),
                ]);
                
                let base = ((addr_high as u64) << 32) | (addr_low as u64);
                *plic_base = Some(base);
            }
        }
    }


    /// Print device tree information for debugging
    pub fn print_info(&self) {
        if self.valid {
            info!("Device Tree Information:");
            info!("  DTB pointer: 0x{:x}", self.dtb_ptr as usize);
            info!("  Status: Valid");
            
            let memory_ranges = self.get_memory_ranges();
            info!("  Memory ranges: {} found", memory_ranges.len());
            for (i, range) in memory_ranges.iter().enumerate() {
                info!("    Range {}: 0x{:x} - 0x{:x} ({} bytes)", 
                      i, range.base, range.base + range.size, range.size);
            }
            
            info!("  CPU count: {}", self.get_cpu_count());
            
            if let Some(freq) = self.get_timer_frequency() {
                info!("  Timer frequency: {} Hz", freq);
            }
            
            if let Some(uart_base) = self.get_uart_base() {
                info!("  UART base: 0x{:x}", uart_base);
            }
            
            if let Some(plic_base) = self.get_plic_base() {
                info!("  PLIC base: 0x{:x}", plic_base);
            }
        } else {
            warn!("No device tree available");
        }
    }
}

/// Global DTB parser instance
static mut DTB_PARSER: Option<DtbParser> = None;

/// Initialize the global DTB parser
pub unsafe fn init(dtb_ptr: *const u8) -> Result<(), &'static str> {
    let parser = unsafe { DtbParser::new(dtb_ptr)? };
    parser.print_info();
    unsafe { DTB_PARSER = Some(parser); }
    Ok(())
}

/// Get a reference to the global DTB parser
#[allow(dead_code)]
pub fn get() -> Option<&'static DtbParser> {
    unsafe { 
        // SAFETY: We ensure that DTB_PARSER is only modified during init
        // and this function is called after init, so it's safe to create a reference
        if let Some(ref parser) = DTB_PARSER {
            Some(parser)
        } else {
            None
        }
    }
}
