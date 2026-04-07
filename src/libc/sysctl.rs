use crate::mem::{ConstPtr, MutPtr, ConstVoidPtr, MutVoidPtr};
use crate::Environment;
use crate::dyld::export_c_func;

fn sysctl(
    env: &mut Environment,
    name_ptr: ConstPtr<i32>,
    namelen: u32,
    _oldp: MutVoidPtr,
    _oldlenp: MutPtr<u32>,
    _newp: MutVoidPtr, 
    _newlen: u32,
) -> i32 {
    let mut mib = vec![0i32; namelen as usize];
    for i in 0..namelen as u32 {
        mib[i as usize] = env.mem.read::<i32, false>(name_ptr + i);
    }
    
    // CRITICAL FIX: Return -1 (Error) so the game knows we didn't fill the buffer.
    // This stops Shift 2 from reading garbage memory and crashing.
    -1 
}

fn sysctlbyname(
    env: &mut Environment,
    name_ptr: ConstPtr<u8>,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<u32>,
    _newp: MutVoidPtr, 
    _newlen: u32,
) -> i32 {
    let name_bytes = env.mem.cstr_at(name_ptr);
    let name_str = String::from_utf8_lossy(name_bytes);
    
    // 1. Device Spoof (Fixes the (null) Phone Model crash)
    if name_str == "hw.machine" || name_str == "hw.model" {
        let hw = b"iPhone4,1\0";
        if !oldlenp.is_null() {
            if oldp.is_null() {
                env.mem.write(oldlenp, hw.len() as u32);
            } else {
                let oldlen = env.mem.read::<u32, false>(oldlenp.cast_const());
                let copy_len = std::cmp::min(oldlen as usize, hw.len());
                env.mem.bytes_at_mut(oldp.cast(), copy_len as u32).copy_from_slice(&hw[..copy_len]);
                env.mem.write(oldlenp, copy_len as u32);
            }
        }
        return 0; // Success
    }

    // 2. EA STOREFRONT BYPASS: Spoof high-end hardware capabilities
    if name_str == "hw.optional.floatingpoint" || name_str == "hw.optional.neon" {
        if !oldlenp.is_null() {
            if oldp.is_null() {
                env.mem.write::<u32>(oldlenp, 4);
            } else {
                env.mem.write::<i32>(oldp.cast(), 1);
                env.mem.write::<u32>(oldlenp, 4);
            }
        }
        return 0; // Success
    }

    // Return -1 for unhandled strings to force EA safe fallbacks
    -1
}

fn __assert_rtn(
    env: &mut Environment,
    func: ConstPtr<u8>,
    file: ConstPtr<u8>,
    line: i32,
    expr: ConstPtr<u8>,
) {
    let func_str = if func.is_null() { "(unknown)".into() } else { String::from_utf8_lossy(env.mem.cstr_at(func)) };
    let file_str = if file.is_null() { "(unknown)".into() } else { String::from_utf8_lossy(env.mem.cstr_at(file)) };
    let expr_str = if expr.is_null() { "(unknown)".into() } else { String::from_utf8_lossy(env.mem.cstr_at(expr)) };
    panic!("\n\nEA GAME ENGINE ASSERTION FAILED!\nFile: {}\nLine: {}\nFunction: {}\nExpression: {}\n\n", file_str, line, func_str, expr_str);
}

pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _)),
    export_c_func!(__assert_rtn(_, _, _, _)),
];
