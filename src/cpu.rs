/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! CPU emulation.
//!
//! Implemented using the C++ library dynarmic, which is a dynamic recompiler.

use crate::abi::GuestFunction;
use crate::mem::{ConstPtr, GuestUSize, Mem, MutPtr, Ptr, SafeRead, SafeWrite};

// Import functions from C++
use touchHLE_dynarmic_wrapper::*;

// ==========================================================
// 🏎️ ARCHITECTURE TOGGLE (32-bit vs 64-bit)
// ==========================================================
#[cfg(not(feature = "aarch64"))]
pub type VAddr = u32;

#[cfg(feature = "aarch64")]
pub type VAddr = u64;

pub type CpuContext = touchHLE_DynarmicContext;

fn touchHLE_cpu_read_impl<T: SafeRead + Default>(
    mem: *mut touchHLE_Mem,
    addr: VAddr,
    error: *mut bool,
) -> T {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mem = unsafe { &mut *mem.cast::<Mem>() };
        // Ptr::from_bits will need to handle 64-bit sizes when AArch64 is active
        let ptr: ConstPtr<T> = Ptr::from_bits(addr); // TODO: AArch64 pointer cast update
        mem.read(ptr)
    }));
    unsafe {
        error.write(res.is_err());
    }
    res.unwrap_or_default()
}

fn touchHLE_cpu_write_impl<T: SafeWrite>(mem: *mut touchHLE_Mem, addr: VAddr, value: T) -> bool {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mem = unsafe { &mut *mem.cast::<Mem>() };
        let ptr: MutPtr<T> = Ptr::from_bits(addr); // TODO: AArch64 pointer cast update
        mem.write(ptr, value)
    }));
    res.is_err()
}

// Export functions for use by C++
#[no_mangle]
extern "C" fn touchHLE_cpu_read_u8(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u8 {
    touchHLE_cpu_read_impl(mem, addr, error)
}
#[no_mangle]
extern "C" fn touchHLE_cpu_read_u16(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u16 {
    touchHLE_cpu_read_impl(mem, addr, error)
}
#[no_mangle]
extern "C" fn touchHLE_cpu_read_u32(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u32 {
    touchHLE_cpu_read_impl(mem, addr, error)
}
#[no_mangle]
extern "C" fn touchHLE_cpu_read_u64(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u64 {
    touchHLE_cpu_read_impl(mem, addr, error)
}
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u8(mem: *mut touchHLE_Mem, addr: VAddr, value: u8) -> bool {
    touchHLE_cpu_write_impl(mem, addr, value)
}
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u16(mem: *mut touchHLE_Mem, addr: VAddr, value: u16) -> bool {
    touchHLE_cpu_write_impl(mem, addr, value)
}
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u32(mem: *mut touchHLE_Mem, addr: VAddr, value: u32) -> bool {
    touchHLE_cpu_write_impl(mem, addr, value)
}
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u64(mem: *mut touchHLE_Mem, addr: VAddr, value: u64) -> bool {
    touchHLE_cpu_write_impl(mem, addr, value)
}

pub struct Cpu {
    dynarmic_wrapper: *mut touchHLE_DynarmicWrapper,
    direct_memory_access_ptr: *const std::ffi::c_void,
}

impl Drop for Cpu {
    fn drop(&mut self) {
        unsafe { touchHLE_DynarmicWrapper_delete(self.dynarmic_wrapper) }
    }
}

#[derive(Debug)]
pub enum CpuState {
    Normal,
    Svc(u32),
    Error(CpuError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CpuError {
    MemoryError,
    UndefinedInstruction,
    Breakpoint,
}

// ==========================================================
// 🏎️ 32-BIT REGISTERS AND LOGIC (DEFAULT)
// ==========================================================
#[cfg(not(feature = "aarch64"))]
impl Cpu {
    pub const SP: usize = 13;
    pub const LR: usize = 14;
    pub const PC: usize = 15;
    pub const CPSR_THUMB: u32 = 0x00000020;
    pub const CPSR_USER_MODE: u32 = 0x00000010;

    pub fn new(direct_memory_access: Option<&mut Mem>) -> Cpu {
        let null_page_count: usize = direct_memory_access
            .as_ref()
            .map_or(0, |mem| mem.null_segment_size() / 0x1000)
            .try_into()
            .unwrap();
        let direct_memory_access_ptr = direct_memory_access
            .map_or(std::ptr::null_mut(), |mem| unsafe {
                mem.direct_memory_access_ptr()
            });
        let dynarmic_wrapper =
            unsafe { touchHLE_DynarmicWrapper_new(direct_memory_access_ptr, null_page_count) };
        Cpu { dynarmic_wrapper, direct_memory_access_ptr }
    }

    pub fn regs(&self) -> &[u32; 16] {
        unsafe {
            let ptr = touchHLE_DynarmicWrapper_regs_const(self.dynarmic_wrapper);
            &*(ptr as *const [u32; 16])
        }
    }
    
    pub fn regs_mut(&mut self) -> &mut [u32; 16] {
        unsafe {
            let ptr = touchHLE_DynarmicWrapper_regs_mut(self.dynarmic_wrapper);
            &mut *(ptr as *mut [u32; 16])
        }
    }

    pub fn dump_regs(&self) {
        Self::echo_regs(self.regs());
    }

    pub fn echo_regs(regs: &[u32; 16]) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for row in 0..4 {
                use std::fmt::Write;
                let mut line = String::new();
                for col in 0..4 {
                    let reg_idx = row * 4 + col;
                    match reg_idx {
                        Self::SP => write!(&mut line, "\t SP: "),
                        Self::LR => write!(&mut line, "\t LR: "),
                        Self::PC => write!(&mut line, "\t PC: "),
                        _ if reg_idx <= 9 => write!(&mut line, "\t R{reg_idx}: "),
                        _ => write!(&mut line, "\tR{reg_idx}: "),
                    }
                    .unwrap();
                    write!(&mut line, "{:#010x}", regs[reg_idx]).unwrap();
                }
                echo!("{}", line);
            }
        }));
    }

    pub fn cpsr(&self) -> u32 {
        unsafe { touchHLE_DynarmicWrapper_cpsr(self.dynarmic_wrapper) }
    }
    
    pub fn set_cpsr(&mut self, cpsr: u32) {
        unsafe { touchHLE_DynarmicWrapper_set_cpsr(self.dynarmic_wrapper, cpsr) }
    }

    pub fn pc_with_thumb_bit(&self) -> GuestFunction {
        let pc = self.regs()[Self::PC];
        let thumb = (self.cpsr() & Self::CPSR_THUMB) == Self::CPSR_THUMB;
        GuestFunction::from_addr_and_thumb_flag(pc, thumb)
    }

    pub fn branch(&mut self, new_pc: GuestFunction) {
        self.regs_mut()[Self::PC] = new_pc.addr_without_thumb_bit();
        let cpsr_without_thumb = self.cpsr() & (!Self::CPSR_THUMB);
        self.set_cpsr(cpsr_without_thumb | ((new_pc.is_thumb() as u32) * Self::CPSR_THUMB))
    }

    pub fn branch_with_link(
        &mut self,
        new_pc: GuestFunction,
        new_lr: GuestFunction,
    ) -> (GuestFunction, GuestFunction) {
        let old_pc = self.pc_with_thumb_bit();
        let old_lr = GuestFunction::from_addr_with_thumb_bit(self.regs()[Self::LR]);
        self.branch(new_pc);
        self.regs_mut()[Self::LR] = new_lr.addr_with_thumb_bit();
        (old_pc, old_lr)
    }
}

// ==========================================================
// 🏎️ 64-BIT REGISTERS AND LOGIC (AARCH64)
// ==========================================================
#[cfg(feature = "aarch64")]
impl Cpu {
    // AArch64 uses x0-x30, plus separate SP and PC.
    // Assuming the C++ wrapper exposes an array of 33 u64s (31 general + SP + PC).
    pub const SP: usize = 31;
    pub const PC: usize = 32;
    pub const LR: usize = 30; // x30 is the link register in AArch64
    
    // AArch64 uses PSTATE instead of CPSR
    pub const PSTATE_EL0: u64 = 0x00000000;

    pub fn new(direct_memory_access: Option<&mut Mem>) -> Cpu {
        // Implementation remains similar, but calls an A64 wrapper variant
        let null_page_count: usize = direct_memory_access
            .as_ref()
            .map_or(0, |mem| mem.null_segment_size() / 0x1000)
            .try_into()
            .unwrap();
        let direct_memory_access_ptr = direct_memory_access
            .map_or(std::ptr::null_mut(), |mem| unsafe {
                mem.direct_memory_access_ptr()
            });
            
        // NOTE: You must update `touchHLE_dynarmic_wrapper.cpp` to support A64 initialization!
        let dynarmic_wrapper =
            unsafe { touchHLE_DynarmicWrapper_new_a64(direct_memory_access_ptr, null_page_count) };
        Cpu { dynarmic_wrapper, direct_memory_access_ptr }
    }

    pub fn regs(&self) -> &[u64; 33] {
        unsafe {
            // 🏎️ Use the A64 specific FFI call
            let ptr = touchHLE_DynarmicWrapper_regs_const_a64(self.dynarmic_wrapper);
            &*(ptr as *const [u64; 33])
        }
    }
    
    pub fn regs_mut(&mut self) -> &mut [u64; 33] {
        unsafe {
            // 🏎️ Use the A64 specific FFI call
            let ptr = touchHLE_DynarmicWrapper_regs_mut_a64(self.dynarmic_wrapper);
            &mut *(ptr as *mut [u64; 33])
        }
    }

    pub fn dump_regs(&self) {
        Self::echo_regs(self.regs());
    }

    pub fn echo_regs(regs: &[u64; 33]) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Updated dump logic for 33 64-bit registers
            for i in 0..31 {
                echo!("\t X{}: {:#018x}", i, regs[i]);
            }
            echo!("\t SP: {:#018x}", regs[Self::SP]);
            echo!("\t PC: {:#018x}", regs[Self::PC]);
        }));
    }

    pub fn pstate(&self) -> u32 {
        // Assuming you add an A64 PSTATE getter to your C++ wrapper
        unsafe { touchHLE_DynarmicWrapper_pstate(self.dynarmic_wrapper) }
    }

    pub fn branch(&mut self, new_pc: u64) {
        self.regs_mut()[Self::PC] = new_pc;
        // AArch64 doesn't use the thumb bit in the PC
    }

    pub fn branch_with_link(&mut self, new_pc: u64, new_lr: u64) -> (u64, u64) {
        let old_pc = self.regs()[Self::PC];
        let old_lr = self.regs()[Self::LR];
        self.branch(new_pc);
        self.regs_mut()[Self::LR] = new_lr;
        (old_pc, old_lr)
    }
}

// ==========================================================
// 🏎️ SHARED LOGIC (BOTH ARCHITECTURES)
// ==========================================================
impl Cpu {
    pub fn swap_context(&mut self, context: &mut CpuContext) {
        unsafe { touchHLE_DynarmicWrapper_swap_context(self.dynarmic_wrapper, context) }
    }

    pub fn invalidate_cache_range(&mut self, base: VAddr, size: GuestUSize) {
        unsafe {
            // Remove the 'as u64' cast and use 'as _' to let the compiler infer the 
            // correct FFI type depending on whether you are in 32-bit or 64-bit mode!
            touchHLE_DynarmicWrapper_invalidate_cache_range(self.dynarmic_wrapper, base as _, size)
        }
    }

    #[must_use]
    pub fn run_or_step(&mut self, mem: &mut Mem, ticks: Option<&mut u64>) -> CpuState {
        if !self.direct_memory_access_ptr.is_null() {
            assert!(self.direct_memory_access_ptr == unsafe { mem.direct_memory_access_ptr() });
        }

        let res = unsafe {
            touchHLE_DynarmicWrapper_run_or_step(
                self.dynarmic_wrapper,
                mem as *mut Mem as *mut touchHLE_Mem,
                ticks,
            )
        };
        match res {
            -1 => CpuState::Normal,
            -2 => CpuState::Error(CpuError::MemoryError),
            -3 => CpuState::Error(CpuError::UndefinedInstruction),
            -4 => CpuState::Error(CpuError::Breakpoint),
            _ if res < -4 => panic!("Unexpected CPU execution result"),
            svc => CpuState::Svc(svc as u32),
        }
    }
}
