/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::cpu::Cpu;
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, Mem, MutPtr, MutVoidPtr, Ptr, SafeRead};
use crate::Environment;

// AArch64 uses X29 for the Frame Pointer
pub const FRAME_POINTER: usize = 29;

#[derive(Copy, Clone, Debug)]
pub struct GuestFunction(ConstVoidPtr);
unsafe impl SafeRead for GuestFunction {}
impl GuestFunction {
    // AArch64 has no Thumb mode
    pub const THUMB_BIT: u64 = 0;

    pub fn from_addr_with_thumb_bit(addr: u64) -> Self {
        GuestFunction(Ptr::from_bits(addr))
    }

    pub fn from_addr_and_thumb_flag(pc: u64, _thumb: bool) -> Self {
        GuestFunction(Ptr::from_bits(pc))
    }

    pub fn is_thumb(self) -> bool { false }
    pub fn addr_with_thumb_bit(self) -> u64 { self.0.to_bits() }
    pub fn addr_without_thumb_bit(self) -> u64 { self.0.to_bits() }
    pub fn to_ptr(self) -> ConstVoidPtr { self.0 }

    pub fn call_without_pushing_stack_frame(self, env: &mut Environment) {
        let (old_pc, old_lr) = env
            .cpu
            .branch_with_link(self.addr_without_thumb_bit(), env.dyld.return_to_host_routine().addr_without_thumb_bit());

        env.run_call();

        env.cpu.branch(old_pc);
        env.cpu.regs_mut()[Cpu::LR] = old_lr;
    }

    pub fn null_ptr() -> Self {
        GuestFunction(Ptr::null())
    }
}

pub trait CallFromGuest {
    fn call_from_guest(&self, env: &mut Environment);
}

macro_rules! impl_CallFromGuest {
    ( $($p:tt => $P:ident),* ) => {
        impl<R, $($P),*> CallFromGuest for fn(&mut Environment, $($P),*) -> R
            where R: GuestRet, $($P: GuestArg,)* {
            #[allow(unused_variables, unused_mut, clippy::unused_unit)]
            fn call_from_guest(&self, env: &mut Environment) {
                let mut reg_offset = 0;
                let regs = env.cpu.regs();
                let retval_ptr = R::SIZE_IN_MEM.map(|_| {
                    read_next_arg(&mut reg_offset, regs, Ptr::from_bits(regs[Cpu::SP]), &env.mem)
                });
                let args: ($($P,)*) = {
                    ($(read_next_arg::<$P>(&mut reg_offset, regs, Ptr::from_bits(regs[Cpu::SP]), &env.mem),)*)
                };
                let retval = self(env, $(args.$p),*);
                if let Some(retval_ptr) = retval_ptr {
                    retval.to_mem(retval_ptr, &mut env.mem);
                } else {
                    retval.to_regs(env.cpu.regs_mut());
                }
            }
        }
        impl<R, $($P),*> CallFromGuest for fn(&mut Environment, $($P,)* DotDotDot) -> R
            where R: GuestRet, $($P: GuestArg,)* {
            #[allow(unused_variables, unused_mut, clippy::unused_unit)]
            fn call_from_guest(&self, env: &mut Environment) {
                let mut reg_offset = 0;
                let regs = env.cpu.regs();
                let retval_ptr = R::SIZE_IN_MEM.map(|_| {
                    read_next_arg(&mut reg_offset, regs, Ptr::from_bits(regs[Cpu::SP]), &env.mem)
                });
                let args: ($($P,)*) = {
                    ($(read_next_arg::<$P>(&mut reg_offset, regs, Ptr::from_bits(regs[Cpu::SP]), &env.mem),)*)
                };
                let va_list = DotDotDot(VaList {
                    reg_offset,
                    stack_pointer: Ptr::from_bits(regs[Cpu::SP])
                });
                let retval = self(env, $(args.$p,)* va_list);
                if let Some(retval_ptr) = retval_ptr {
                    retval.to_mem(retval_ptr, &mut env.mem);
                } else {
                    retval.to_regs(env.cpu.regs_mut());
                }
            }
        }
    }
}

impl_CallFromGuest!();
impl_CallFromGuest!(0 => P0);
impl_CallFromGuest!(0 => P0, 1 => P1);
impl_CallFromGuest!(0 => P0, 1 => P1, 2 => P2);
impl_CallFromGuest!(0 => P0, 1 => P1, 2 => P2, 3 => P3);
impl_CallFromGuest!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4);
impl_CallFromGuest!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5);
impl_CallFromGuest!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5, 6 => P6);
impl_CallFromGuest!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5, 6 => P6, 7 => P7);
impl_CallFromGuest!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5, 6 => P6, 7 => P7, 8 => P8);

pub trait CallFromHost<R, P> {
    fn call_from_host(&self, env: &mut Environment, args: P) -> R;
}

macro_rules! impl_CallFromHost {
    ( $($p:tt => $P:ident),* ) => {
        impl <T, R, $($P),*> CallFromHost<R, ($($P,)*)> for T
            where T: CallFromGuest, R: GuestRet, $($P: GuestArg,)* {
            #[allow(unused_variables, unused_mut, clippy::unused_unit)]
            fn call_from_host(&self, env: &mut Environment, args: ($($P,)*)) -> R {
                let mut reg_offset = 0;
                let regs = env.cpu.regs_mut();
                let retval_ptr = R::SIZE_IN_MEM.map(|size| {
                    regs[Cpu::SP] -= size;
                    let ptr: ConstVoidPtr = Ptr::from_bits(regs[Cpu::SP]);
                    write_next_arg(&mut reg_offset, regs, &mut env.mem, ptr);
                    ptr
                });
                let old_sp = extend_stack_for_args(0 $(+ <$P as GuestArg>::REG_COUNT)*, regs);
                $(write_next_arg::<$P>(&mut reg_offset, regs, &mut env.mem, args.$p);)*
                self.call_from_guest(env);
                let regs = env.cpu.regs_mut(); 
                regs[Cpu::SP] = old_sp;
                if let Some(retval_ptr) = retval_ptr {
                    regs[Cpu::SP] += R::SIZE_IN_MEM.unwrap();
                    <R as GuestRet>::from_mem(retval_ptr, &env.mem)
                } else {
                    <R as GuestRet>::from_regs(regs)
                }
            }
        }

        impl <R, $($P),*> CallFromHost<R, ($($P,)*)> for GuestFunction
            where R: GuestRet, $($P: GuestArg,)* {
            #[allow(unused_variables, unused_mut, clippy::unused_unit)]
            fn call_from_host(&self, env: &mut Environment, args: ($($P,)*)) -> R {
                let (old_pc, old_lr) = env.cpu.branch_with_link(self.addr_without_thumb_bit(), env.dyld.return_to_host_routine().addr_without_thumb_bit());

                let (old_sp, old_fp) = {
                    let regs = env.cpu.regs_mut();
                    let old_sp = regs[Cpu::SP];
                    let old_fp = regs[FRAME_POINTER];
                    regs[Cpu::SP] -= 16;
                    regs[FRAME_POINTER] = regs[Cpu::SP];
                    env.mem.write(Ptr::from_bits(regs[Cpu::SP]), old_fp);
                    env.mem.write(Ptr::from_bits(regs[Cpu::SP] + 8), old_lr);
                    (old_sp, old_fp)
                };

                let regs = env.cpu.regs_mut();
                let _ = extend_stack_for_args(0 $(+ <$P as GuestArg>::REG_COUNT)*, regs);
                let mut reg_offset = 0;
                $(write_next_arg::<$P>(&mut reg_offset, regs, &mut env.mem, args.$p);)*

                env.run_call();
                env.cpu.branch(old_pc);

                let regs = env.cpu.regs_mut();
                regs[Cpu::LR] = old_lr;
                regs[Cpu::SP] = old_sp;
                regs[FRAME_POINTER] = old_fp;
                <R as GuestRet>::from_regs(env.cpu.regs())
            }
        }
    }
}

impl_CallFromHost!();
impl_CallFromHost!(0 => P0);
impl_CallFromHost!(0 => P0, 1 => P1);
impl_CallFromHost!(0 => P0, 1 => P1, 2 => P2);
impl_CallFromHost!(0 => P0, 1 => P1, 2 => P2, 3 => P3);
impl_CallFromHost!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4);
impl_CallFromHost!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5);
impl_CallFromHost!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5, 6 => P6);
impl_CallFromHost!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5, 6 => P6, 7 => P7);
impl_CallFromHost!(0 => P0, 1 => P1, 2 => P2, 3 => P3, 4 => P4, 5 => P5, 6 => P6, 7 => P7, 8 => P8);

pub trait GuestArg: std::fmt::Debug + Sized {
    const REG_COUNT: usize;
    fn from_regs(regs: &[u64]) -> Self;
    fn to_regs(self, regs: &mut [u64]);
}

// AAPCS64 passes up to 8 arguments in registers (X0-X7)
const MAX_ARG_REGS: usize = 8;

fn read_next_arg<T: GuestArg>(
    reg_offset: &mut usize,
    regs: &[u64],
    stack_ptr: ConstPtr<u64>,
    mem: &Mem,
) -> T {
    let mut fake_regs = [0u64; 16];
    let fake_regs = &mut fake_regs[0..T::REG_COUNT];

    for fake_reg in fake_regs.iter_mut() {
        if *reg_offset < MAX_ARG_REGS {
            *fake_reg = regs[*reg_offset];
        } else {
            *fake_reg = mem.read(stack_ptr + (*reg_offset - MAX_ARG_REGS).try_into().unwrap());
        }
        *reg_offset += 1;
    }
    T::from_regs(fake_regs)
}

pub fn extend_stack_for_args(reg_count_sum: usize, regs: &mut [u64]) -> u64 {
    let old = regs[Cpu::SP];
    if reg_count_sum > MAX_ARG_REGS {
        let old_ptr: ConstPtr<u64> = Ptr::from_bits(old);
        regs[Cpu::SP] = (old_ptr - (reg_count_sum - MAX_ARG_REGS).try_into().unwrap()).to_bits();
    }
    old
}

pub fn write_next_arg<T: GuestArg>(
    reg_offset: &mut usize,
    regs: &mut [u64],
    mem: &mut Mem,
    arg: T,
) {
    let mut fake_regs = [0u64; 16];
    let fake_regs = &mut fake_regs[0..T::REG_COUNT];
    arg.to_regs(fake_regs);

    for &mut fake_reg in fake_regs {
        if *reg_offset < MAX_ARG_REGS {
            regs[*reg_offset] = fake_reg;
        } else {
            let stack_ptr: MutPtr<u64> = Ptr::from_bits(regs[Cpu::SP]);
            mem.write(stack_ptr + (*reg_offset - MAX_ARG_REGS).try_into().unwrap(), fake_reg);
        }
        *reg_offset += 1;
    }
}

#[derive(Debug)]
pub struct DotDotDot(VaList);
impl DotDotDot {
    pub fn start(&self) -> VaList { self.0 }
}

#[derive(Copy, Clone, Debug)]
pub struct VaList {
    reg_offset: usize,
    stack_pointer: ConstVoidPtr,
}
impl VaList {
    pub fn next<T: GuestArg>(&mut self, env: &mut Environment) -> T {
        let sp_reg = self.stack_pointer.cast();
        read_next_arg(&mut self.reg_offset, env.cpu.regs_mut(), sp_reg, &env.mem)
    }
}

macro_rules! impl_GuestArg_with {
    ($for:ty, $with:ty) => {
        impl GuestArg for $for {
            const REG_COUNT: usize = <$with as GuestArg>::REG_COUNT;
            fn from_regs(regs: &[u64]) -> Self { <$with as GuestArg>::from_regs(regs) as $for }
            fn to_regs(self, regs: &mut [u64]) { <$with as GuestArg>::to_regs(self as $with, regs) }
        }
    };
}

// ----------------------------------------------------
// 🏎️ 64-BIT ARGUMENTS AND RETURNS
// ----------------------------------------------------

impl GuestArg for u64 {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self { regs[0] }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self; }
}
impl_GuestArg_with!(i64, u64);

impl GuestArg for f64 {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self { Self::from_bits(regs[0]) }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self.to_bits(); }
}

impl GuestArg for u32 {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self { regs[0] as u32 }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self as u64; }
}
impl_GuestArg_with!(i32, u32);
impl_GuestArg_with!(u16, u32);
impl_GuestArg_with!(i16, u32);
impl_GuestArg_with!(u8, u32);
impl_GuestArg_with!(i8, u32);

impl GuestArg for bool {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self { regs[0] != 0 }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self as u64; }
}

impl GuestArg for f32 {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self { Self::from_bits(regs[0] as u32) }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self.to_bits() as u64; }
}

impl<T, const MUT: bool> GuestArg for Ptr<T, MUT> {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self { Self::from_bits(regs[0]) }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self.to_bits(); }
}

impl GuestArg for GuestFunction {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self { GuestFunction(<ConstVoidPtr as GuestArg>::from_regs(regs)) }
    fn to_regs(self, regs: &mut [u64]) { <ConstVoidPtr as GuestArg>::to_regs(self.0, regs) }
}

impl GuestArg for VaList {
    const REG_COUNT: usize = 1;
    fn from_regs(regs: &[u64]) -> Self {
        VaList {
            reg_offset: 8,
            stack_pointer: <ConstVoidPtr as GuestArg>::from_regs(regs),
        }
    }
    fn to_regs(self, _regs: &mut [u64]) { todo!() }
}

pub trait GuestRet: std::fmt::Debug + Sized {
    const SIZE_IN_MEM: Option<GuestUSize> = None;
    fn from_regs(regs: &[u64]) -> Self { panic!() }
    fn to_regs(self, regs: &mut [u64]) { panic!() }
    fn from_mem(ptr: ConstVoidPtr, mem: &Mem) -> Self { panic!() }
    fn to_mem(self, ptr: MutVoidPtr, mem: &mut Mem) { panic!() }
}

macro_rules! impl_GuestRet_with {
    ($for:ty, $with:ty) => {
        impl GuestRet for $for {
            fn from_regs(regs: &[u64]) -> Self { <$with as GuestRet>::from_regs(regs) as $for }
            fn to_regs(self, regs: &mut [u64]) { <$with as GuestRet>::to_regs(self as $with, regs) }
        }
    };
}

#[macro_export]
macro_rules! impl_GuestRet_for_large_struct {
    ($for:ty) => {
        impl $crate::abi::GuestRet for $for {
            const SIZE_IN_MEM: Option<$crate::mem::GuestUSize> = Some($crate::mem::guest_size_of::<$for>());
            fn from_mem(ptr: $crate::mem::ConstVoidPtr, mem: &$crate::mem::Mem) -> Self { mem.read(ptr.cast::<Self>()) }
            fn to_mem(self, ptr: $crate::mem::MutVoidPtr, mem: &mut $crate::mem::Mem) { mem.write(ptr.cast::<Self>(), self) }
        }
    };
}
pub use crate::impl_GuestRet_for_large_struct;

impl GuestRet for () {
    fn to_regs(self, _regs: &mut [u64]) {}
    fn from_regs(_regs: &[u64]) -> Self {}
}

impl GuestRet for u64 {
    fn from_regs(regs: &[u64]) -> Self { regs[0] }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self; }
}
impl_GuestRet_with!(i64, u64);

impl GuestRet for f64 {
    fn from_regs(regs: &[u64]) -> Self { Self::from_bits(regs[0]) }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self.to_bits(); }
}

impl GuestRet for u32 {
    fn from_regs(regs: &[u64]) -> Self { regs[0] as u32 }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self as u64; }
}
impl_GuestRet_with!(i32, u32);
impl_GuestRet_with!(u16, u32);
impl_GuestRet_with!(i16, u32);
impl_GuestRet_with!(u8, u32);
impl_GuestRet_with!(i8, u32);

impl GuestRet for bool {
    fn from_regs(regs: &[u64]) -> Self { regs[0] != 0 }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self as u64; }
}

impl GuestRet for f32 {
    fn from_regs(regs: &[u64]) -> Self { Self::from_bits(regs[0] as u32) }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self.to_bits() as u64; }
}

impl<T, const MUT: bool> GuestRet for Ptr<T, MUT> {
    fn from_regs(regs: &[u64]) -> Self { Self::from_bits(regs[0]) }
    fn to_regs(self, regs: &mut [u64]) { regs[0] = self.to_bits(); }
}
