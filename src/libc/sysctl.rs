use crate::mem::{ConstPtr, MutPtr, ConstVoidPtr, MutVoidPtr};
use crate::Environment;
use crate::dyld::export_c_func;

fn sysctl(
    env: &mut Environment,
    name_ptr: ConstPtr<i32>,
    namelen: u32,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<u32>,
    _newp: MutVoidPtr, 
    _newlen: u32,
) -> i32 {
    let mut mib = vec![0i32; namelen as usize];
    for i in 0..namelen as u32 {
        mib[i as usize] = env.mem.read::<i32, false>(name_ptr + i);
    }
    
    // 🛡️ THE MAC ADDRESS SPOOF (Fixes EA ValidateDeviceId natively)
    if mib.len() >= 5 && mib[0] == 4 && mib[1] == 17 && mib[3] == 18 && mib[4] == 3 {
        let req_size = 152;
        if oldp.is_null() && !oldlenp.is_null() {
            env.mem.write::<u32>(oldlenp, req_size);
            return 0;
        } else if !oldp.is_null() && !oldlenp.is_null() {
            let len = env.mem.read::<u32, false>(oldlenp.cast_const());
            if len >= 93 { 
                let buf = env.mem.bytes_at_mut(oldp.cast(), len);
                buf.fill(0);
                
                buf[0] = 76; buf[2] = 5; buf[3] = 14; 
                buf[76] = 20; buf[77] = 18; buf[80] = 6; 
                buf[81] = 3; buf[82] = 6; 
                buf[84] = b'e'; buf[85] = b'n'; buf[86] = b'0';
                buf[87] = 0x02; buf[88] = 0x11; buf[89] = 0x22; 
                buf[90] = 0x33; buf[91] = 0x44; buf[92] = 0x55;
                
                env.mem.write::<u32>(oldlenp, len);
                return 0;
            }
        }
    }
    
    // 🛡️ CRITICAL FIX: Return -1 for everything else to prevent EA C++ asserts!
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
        return 0;
    }

    if name_str == "hw.optional.floatingpoint" || name_str == "hw.optional.neon" {
        if !oldlenp.is_null() {
            if oldp.is_null() {
                env.mem.write::<u32>(oldlenp, 4);
            } else {
                env.mem.write::<i32>(oldp.cast(), 1);
                env.mem.write::<u32>(oldlenp, 4);
            }
        }
        return 0;
    }

    if name_str == "hw.memsize" || name_str == "hw.physmem" || name_str == "hw.usermem" {
        if !oldlenp.is_null() {
            if oldp.is_null() {
                env.mem.write::<u32>(oldlenp, 4);
            } else {
                env.mem.write::<u32>(oldp.cast(), 536870912);
                env.mem.write::<u32>(oldlenp, 4);
            }
        }
        return 0;
    }

    // 🛡️ CRITICAL FIX: Return -1 for unknown strings to prevent EA C++ asserts!
    -1
}

fn CGFontGetUnitsPerEm(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { 1000 }
fn CGFontGetAscent(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { 800 }
fn CGFontGetDescent(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { -200 }
fn CGFontRetain(_env: &mut Environment, font: ConstVoidPtr) -> ConstVoidPtr { font }
fn CGFontCreateWithDataProvider(_env: &mut Environment, provider: ConstVoidPtr) -> ConstVoidPtr { provider }
fn CGDataProviderCreateSequential(_env: &mut Environment, info: ConstVoidPtr, _callbacks: ConstVoidPtr) -> ConstVoidPtr {
    if info.is_null() { crate::mem::Ptr::from_bits(1) } else { info }
}

fn SCNetworkReachabilityCreateWithAddress(_env: &mut Environment, _allocator: ConstVoidPtr, _address: ConstVoidPtr) -> ConstVoidPtr {
    crate::mem::Ptr::from_bits(0xDEADBEEF) 
}

fn SCNetworkReachabilityCreateWithName(_env: &mut Environment, _allocator: ConstVoidPtr, _nodename: ConstVoidPtr) -> ConstVoidPtr {
    crate::mem::Ptr::from_bits(0xDEADBEEF) 
}

fn SCNetworkReachabilityGetFlags(env: &mut Environment, _target: ConstVoidPtr, flags_out: MutPtr<u32>) -> i32 {
    if !flags_out.is_null() {
        env.mem.write::<u32>(flags_out, 0); 
    }
    1
}

fn __srget(_env: &mut Environment, _fp: ConstVoidPtr) -> i32 { 0 }
fn flockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }
fn funlockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }

fn xmlFree(_env: &mut Environment, _ptr: ConstVoidPtr) {}

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
    panic!("EA GAME ENGINE ASSERTION FAILED!\nFile: {}\nLine: {}\nFunction: {}\nExpression: {}", file_str, line, func_str, expr_str);
}

fn object_getClass(env: &mut Environment, obj: ConstVoidPtr) -> ConstVoidPtr {
    if obj.is_null() { return crate::mem::Ptr::null(); }
    if obj.to_bits() == 0xDEADBEEF { return crate::mem::Ptr::from_bits(0x30000000); }
    let isa = env.mem.read::<u32, false>(obj.cast());
    crate::mem::Ptr::from_bits(isa)
}

fn class_getProperty(_env: &mut Environment, _cls: ConstVoidPtr, _name: ConstVoidPtr) -> ConstVoidPtr {
    crate::mem::Ptr::null() 
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
    export_c_func!(xmlFree(_)),
    export_c_func!(__assert_rtn(_, _, _, _)),
    export_c_func!(object_getClass(_)),
    export_c_func!(class_getProperty(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _)),
];
