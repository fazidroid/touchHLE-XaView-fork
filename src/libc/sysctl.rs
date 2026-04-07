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
    log!("GAMELOFT BYPASS: sysctl called for mib {:?}, faking success", mib);
    0 
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
    
    // GLES 2.0 ENABLER: Force games to think this is an iPhone 4S
    if name_str == "hw.machine" {
        log!("GAMELOFT/EA BYPASS: Forcing hw.machine to iPhone4,1");
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

    log!("GAMELOFT/EA BYPASS: sysctlbyname '{}', faking hardware success", name_str);
    0
}

// ==== NFS SHIFT 2 FONT CRASH BYPASSES ====
// These return dummy math values so the game doesn't divide-by-zero
fn CGFontGetUnitsPerEm(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { 1000 }
fn CGFontGetAscent(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { 800 }
fn CGFontGetDescent(_env: &mut Environment, _font: ConstVoidPtr) -> i32 { -200 }
fn CGFontRetain(_env: &mut Environment, font: ConstVoidPtr) -> ConstVoidPtr { font }
fn CGFontCreateWithDataProvider(_env: &mut Environment, provider: ConstVoidPtr) -> ConstVoidPtr { provider }

// NEW: Bypass for Custom Font Loading Data Provider
// Returns a safe dummy pointer so the game doesn't crash on NULL
fn CGDataProviderCreateSequential(_env: &mut Environment, info: ConstVoidPtr, _callbacks: ConstVoidPtr) -> ConstVoidPtr {
    if info.is_null() { crate::mem::Ptr::from_bits(1) } else { info }
}

// ==== NFS SHIFT 2 FILE I/O INFINITE LOOP BYPASS ====
// Returns EOF (-1) so the game knows when to stop reading empty files
fn ___srget(_env: &mut Environment, _fp: ConstVoidPtr) -> i32 { -1 }

// NEW: File lock thread-safety bypasses (prevents infinite loop fallback)
fn flockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }
fn funlockfile(_env: &mut Environment, _file: ConstVoidPtr) -> i32 { 0 }

pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _)),
    
    // Shift 2 Font Bypasses
    export_c_func!(CGFontGetUnitsPerEm(_)),
    export_c_func!(CGFontGetAscent(_)),
    export_c_func!(CGFontGetDescent(_)),
    export_c_func!(CGFontRetain(_)),
    export_c_func!(CGFontCreateWithDataProvider(_)),
    export_c_func!(CGDataProviderCreateSequential(_, _)),
    
    // Shift 2 File I/O Bypass
    export_c_func!(___srget(_)),
    export_c_func!(flockfile(_)),
    export_c_func!(funlockfile(_)),
];
