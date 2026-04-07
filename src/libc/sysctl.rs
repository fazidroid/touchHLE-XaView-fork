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
        // FIXED: Removed .unwrap_or(0) because read() returns the i32 directly
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
    
    // GLES 2.0 ENABLER: Force Gameloft/EA games to think this is an iPhone 4S
    if name_str == "hw.machine" {
        log!("GAMELOFT BYPASS: Forcing hw.machine to iPhone4,1 (Enables GLES 2.0)");
        let hw = b"iPhone4,1\0";
        if !oldp.is_null() && !oldlenp.is_null() {
            // FIXED: Removed .unwrap_or(0) because read() returns the u32 directly
            let oldlen = env.mem.read::<u32, false>(oldlenp.cast_const());
            if oldlen as usize >= hw.len() {
                env.mem.bytes_at_mut(oldp.cast(), hw.len() as u32).copy_from_slice(hw);
            }
            env.mem.write(oldlenp, hw.len() as u32);
        }
        return 0;
    }

    log!("GAMELOFT BYPASS: sysctlbyname '{}', faking hardware success", name_str);
    0
}

pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _)),
];
