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
    _oldp: MutVoidPtr,
    _oldlenp: MutPtr<u32>,
    _newp: ConstVoidPtr,
    _newlen: u32,
) -> i32 {
    let name_bytes = env.mem.cstr_at(name_ptr);
    let name_str = String::from_utf8_lossy(name_bytes);
    log!("GAMELOFT BYPASS: sysctlbyname '{}', faking hardware success", name_str);
    0
}

pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _, _)),
];
