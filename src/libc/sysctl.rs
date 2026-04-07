use crate::mem::{ConstPtr, MutPtr, ConstVoidPtr, MutVoidPtr};
use crate::Environment;
use crate::dyld::export_c_func;

fn sysctl(
    env: &mut Environment,
    name_ptr: ConstPtr<i32>,
    namelen: u32,
    _oldp: MutVoidPtr,
    _oldlenp: MutPtr<u32>,
    _newp: ConstVoidPtr,
    _newlen: u32,
) -> i32 {
    let mut mib = vec![0i32; namelen as usize];
    for i in 0..namelen as usize {
        mib[i] = env.mem.read::<i32, false>(name_ptr + (i as u32));
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
                // Step 1: The game is only asking for the SIZE of the string
                env.mem.write(oldlenp, hw.len() as u32);
            } else {
                // Step 2: The game allocated the memory and wants the ACTUAL string
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

pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    // Fixed: sysctl has 6 args after env
    export_c_func!(sysctl(_, _, _, _, _, _)),
    // Fixed: sysctlbyname has 5 args after env
    export_c_func!(sysctlbyname(_, _, _, _, _)),
];
