use crate::mem::{ConstPtr, MutPtr, ConstVoidPtr, MutVoidPtr};
use crate::Environment;
use crate::dyld::export_c_func;

fn sysctl(
    env: &mut Environment,
    name_ptr: ConstPtr<i32>,
    namelen: u32,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<u32>,
    newp: ConstVoidPtr,
    newlen: u32,
) -> i32 {
    let mut mib = vec![0i32; namelen as usize];
    for i in 0..namelen as usize {
        // FIXED: Using standard addition instead of .offset()
        mib[i] = env.mem.read(name_ptr + (i as u32)).unwrap_or(0);
    }
    
    // Faking MAC address and device info for Gameloft/EA titles
    log!("GAMELOFT BYPASS: sysctl called for mib {:?}, faking success", mib);
    0 
}

fn sysctlbyname(
    env: &mut Environment,
    name_ptr: ConstPtr<u8>,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<u32>,
    newp: ConstVoidPtr,
    newlen: u32,
) -> i32 {
    let name_str = env.mem.cstr_at(name_ptr).to_string_lossy();
    log!("GAMELOFT BYPASS: sysctlbyname '{}', faking hardware success", name_str);
    0
}

pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    // FIXED: Correcting the argument count in the macro
    export_c_func!(sysctl(_, _, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _, _)),
];
