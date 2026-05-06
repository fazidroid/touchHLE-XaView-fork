/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use crate::abi::GuestFunction;
use crate::mem::{ConstPtr, GuestUSize, Mem, MutPtr, Ptr, SafeRead, SafeWrite};
use touchHLE_dynarmic_wrapper::*;

// 🏎️ PERMANENT 64-BIT VIRTUAL ADDRESSING
pub type VAddr = u64; 
pub type CpuContext = touchHLE_DynarmicContext;

fn touchHLE_cpu_read_impl<T: SafeRead + Default>(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> T {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mem = unsafe { &mut *mem.cast::<Mem>() };
        let ptr: ConstPtr<T> = Ptr::from_bits(addr); 
        mem.read(ptr)
    }));
    unsafe { error.write(res.is_err()); }
    res.unwrap_or_default()
}

fn touchHLE_cpu_write_impl<T: SafeWrite>(mem: *mut touchHLE_Mem, addr: VAddr, value: T) -> bool {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mem = unsafe { &mut *mem.cast::<Mem>() };
        let ptr: MutPtr<T> = Ptr::from_bits(addr); 
        mem.write(ptr, value)
    }));
    res.is_err()
}

#[no_mangle]
extern "C" fn touchHLE_cpu_read_u8(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u8 { touchHLE_cpu_read_impl(mem, addr, error) }
#[no_mangle]
extern "C" fn touchHLE_cpu_read_u16(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u16 { touchHLE_cpu_read_impl(mem, addr, error) }
#[no_mangle]
extern "C" fn touchHLE_cpu_read_u32(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u32 { touchHLE_cpu_read_impl(mem, addr, error) }
#[no_mangle]
extern "C" fn touchHLE_cpu_read_u64(mem: *mut touchHLE_Mem, addr: VAddr, error: *mut bool) -> u64 { touchHLE_cpu_read_impl(mem, addr, error) }
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u8(mem: *mut touchHLE_Mem, addr: VAddr, value: u8) -> bool { touchHLE_cpu_write_impl(mem, addr, value) }
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u16(mem: *mut touchHLE_Mem, addr: VAddr, value: u16) -> bool { touchHLE_cpu_write_impl(mem, addr, value) }
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u32(mem: *mut touchHLE_Mem, addr: VAddr, value: u32) -> bool { touchHLE_cpu_write_impl(mem, addr, value) }
#[no_mangle]
extern "C" fn touchHLE_cpu_write_u64(mem: *mut touchHLE_Mem, addr: VAddr, value: u64) -> bool { touchHLE_cpu_write_impl(mem, addr, value) }

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
pub enum CpuState { Normal, Svc(u32), Error(CpuError) }

#[derive(Debug, Clone, PartialEq)]
pub enum CpuError { MemoryError, UndefinedInstruction, Breakpoint }

// ==========================================================
// 🏎️ 64-BIT REGISTERS ONLY
// ==========================================================
impl Cpu {
    pub const SP: usize = 31;
    pub const PC: usize = 32;
    pub const LR: usize = 30; 
    pub const PSTATE_EL0: u64 = 0x00000000;

    pub fn new(direct_memory_access: Option<&mut Mem>) -> Cpu {
        let null_page_count: usize = direct_memory_access.as_ref().map_or(0, |mem| mem.null_segment_size() / 0x1000).try_into().unwrap();
        let direct_memory_access_ptr = direct_memory_access.map_or(std::ptr::null_mut(), |mem| unsafe { mem.direct_memory_access_ptr() });
        let dynarmic_wrapper = unsafe { touchHLE_DynarmicWrapper_new_a64(direct_memory_access_ptr, null_page_count) };
        Cpu { dynarmic_wrapper, direct_memory_access_ptr }
    }

    pub fn regs(&self) -> &[u64; 33] {
        unsafe { &*(touchHLE_DynarmicWrapper_regs_const_a64(self.dynarmic_wrapper) as *const [u64; 33]) }
    }
    
    pub fn regs_mut(&mut self) -> &mut [u64; 33] {
        unsafe { &mut *(touchHLE_DynarmicWrapper_regs_mut_a64(self.dynarmic_wrapper) as *mut [u64; 33]) }
    }

    pub fn dump_regs(&self) { Self::echo_regs(self.regs()); }

    pub fn echo_regs(regs: &[u64; 33]) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for i in 0..31 { echo!("\t X{}: {:#018x}", i, regs[i]); }
            echo!("\t SP: {:#018x}", regs[Self::SP]);
            echo!("\t PC: {:#018x}", regs[Self::PC]);
        }));
    }

    pub fn pstate(&self) -> u32 { unsafe { touchHLE_DynarmicWrapper_pstate(self.dynarmic_wrapper) } }

    pub fn branch(&mut self, new_pc: u64) {
        self.regs_mut()[Self::PC] = new_pc;
    }

    pub fn branch_with_link(&mut self, new_pc: u64, new_lr: u64) -> (u64, u64) {
        let old_pc = self.regs()[Self::PC];
        let old_lr = self.regs()[Self::LR];
        self.regs_mut()[Self::PC] = new_pc;
        self.regs_mut()[Self::LR] = new_lr;
        (old_pc, old_lr)
    }

    pub fn swap_context(&mut self, context: &mut CpuContext) {
        unsafe { touchHLE_DynarmicWrapper_swap_context(self.dynarmic_wrapper, context) }
    }

    pub fn invalidate_cache_range(&mut self, base: VAddr, size: GuestUSize) {
        unsafe { touchHLE_DynarmicWrapper_invalidate_cache_range_a64(self.dynarmic_wrapper, base, size as u32) }
    }

    #[must_use]
    pub fn run_or_step(&mut self, mem: &mut Mem, ticks: Option<&mut u64>) -> CpuState {
        let res = unsafe { touchHLE_DynarmicWrapper_run_or_step_a64(self.dynarmic_wrapper, mem as *mut Mem as *mut touchHLE_Mem, ticks) };
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
