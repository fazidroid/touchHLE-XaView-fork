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
    
    //  THE MAC ADDRESS / DEVICE ID SPOOF (Fixes ValidateDeviceId)
    // By returning 0 and zeroing the buffer, EA's crypto-hasher hashes a clean 00:00:00 MAC.
    if !oldp.is_null() && !oldlenp.is_null() {
        let len = env.mem.read::<u32, false>(oldlenp.cast_const());
        if len > 0 {
            let buf = env.mem.bytes_at_mut(oldp.cast(), len);
            buf.fill(0);
        }
    }
    
    0 // Return Success
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
    
    // 1. Device Spoof
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

    // 2. EA STOREFRONT BYPASS
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

    // 3. RAM SPOOF (Fixes memory assertion crashes)
    if name_str == "hw.memsize" || name_str == "hw.physmem" || name_str == "hw.usermem" {
        if !oldlenp.is_null() {
            if oldp.is_null() {
                env.mem.write::<u32>(oldlenp, 4);
            } else {
                env.mem.write::<u32>(oldp.cast(), 536870912); // Tell EA we have 512 MB RAM
                env.mem.write::<u32>(oldlenp, 4);
            }
        }
        return 0;
    }

    // 4. UNIVERSAL SUCCESS FALLBACK
    if !oldp.is_null() && !oldlenp.is_null() {
        let len = env.mem.read::<u32, false>(oldlenp.cast_const());
        if len > 0 {
            let buf = env.mem.bytes_at_mut(oldp.cast(), len);
            buf.fill(0);
        }
    }
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

// ==== SYSTEM CONFIGURATION / NETWORK REACHABILITY BYPASS ====
fn SCNetworkReachabilityCreateWithAddress(_env: &mut Environment, _allocator: ConstVoidPtr, _address: ConstVoidPtr) -> ConstVoidPtr {
    crate::mem::Ptr::from_bits(0xDEADBEEF) 
}

fn SCNetworkReachabilityCreateWithName(_env: &mut Environment, _allocator: ConstVoidPtr, _nodename: ConstVoidPtr) -> ConstVoidPtr {
    crate::mem::Ptr::from_bits(0xDEADBEEF) 
}

fn SCNetworkReachabilityGetFlags(env: &mut Environment, _target: ConstVoidPtr, flags_out: MutPtr<u32>) -> i32 {
    if !flags_out.is_null() {
        env.mem.write::<u32>(flags_out, 2); 
    }
    1
}

// ==== FILE I/O BYPASS ====
fn __srget(_env: &mut Environment, _fp: ConstVoidPtr) -> i32 { -1 }
fn flockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }
fn funlockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }

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
    
    crate::log!("\n\n EA ASSERTION BYPASSED!\nFile: {}\nLine: {}\nFunction: {}\nExpression: {}\nEngine tried to crash but was denied!\n\n", file_str, line, func_str, expr_str);
}

// ==== OBJECTIVE-C RUNTIME FIXES ====
fn object_getClass(env: &mut Environment, obj: ConstVoidPtr) -> ConstVoidPtr {
    if obj.is_null() { 
        return crate::mem::Ptr::null(); 
    }
    
    if obj.to_bits() == 0xDEADBEEF {
        return crate::mem::Ptr::from_bits(0x30000000); 
    }

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
    
    export_c_func!(__assert_rtn(_, _, _, _)),
    
    export_c_func!(object_getClass(_)),
    export_c_func!(class_getProperty(_, _)),

    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _)),
];
