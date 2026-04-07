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
    
    // 1. Device Spoof (Catches hw.model so Phone Model is no longer (null))
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

    // Return -1 for unhandled strings to force EA safe fallbacks
    -1
}

    // NEW: Spoof capability check (Required for EA MTX Controller)
    if name_str == "hw.optional.floatingpoint" || name_str == "hw.optional.neon" {
        log!("EA BYPASS: Spoofing {} capability", name_str);
        if !oldp.is_null() && !oldlenp.is_null() {
            env.mem.write::<i32>(oldp.cast(), 1);
            env.mem.write::<u32>(oldlenp, 4);
        }
        return 0;
    }

    log!("GAMELOFT/EA BYPASS: sysctlbyname '{}', faking hardware success", name_str);
    0
}

// ==== FONT CRASH BYPASSES ====
fn CGFontGetUnitsPerEm(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { 1000 }
fn CGFontGetAscent(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { 800 }
fn CGFontGetDescent(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { -200 }
fn CGFontRetain(_env: &mut Environment, font: ConstVoidPtr) -> ConstVoidPtr { font }
fn CGFontCreateWithDataProvider(_env: &mut Environment, provider: ConstVoidPtr) -> ConstVoidPtr { provider }
fn CGDataProviderCreateSequential(_env: &mut Environment, info: ConstVoidPtr, _callbacks: ConstVoidPtr) -> ConstVoidPtr {
    if info.is_null() { crate::mem::Ptr::from_bits(1) } else { info }
}

// ==== FILE I/O INFINITE LOOP BYPASS ====
fn __srget(_env: &mut Environment, _fp: ConstVoidPtr) -> i32 { -1 }
fn flockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }
fn funlockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }

// ==== NEW: EA GAME ENGINE ASSERTION REVEALER ====
// This intercepts the game's fatal crashes and prints the exact reason to the log
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
    
    export_c_func!(CGFontGetUnitsPerEm(_)),
    export_c_func!(CGFontGetAscent(_)),
    export_c_func!(CGFontGetDescent(_)),
    export_c_func!(CGFontRetain(_)),
    export_c_func!(CGFontCreateWithDataProvider(_)),
    export_c_func!(CGDataProviderCreateSequential(_, _)),
    
    export_c_func!(__srget(_)),
    export_c_func!(flockfile(_)),
    export_c_func!(funlockfile(_)),
    
    // Export the new assertion revealer
    export_c_func!(__assert_rtn(_, _, _, _)),
];
