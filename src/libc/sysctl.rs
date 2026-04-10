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
    
    //  THE MAC ADDRESS SPOOF (Fixes EA ValidateDeviceId natively)
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
                buf[87] = 0x02;
                buf[88] = 0x11; buf[89] = 0x22; 
                buf[90] = 0x33; buf[91] = 0x44; buf[92] = 0x55;
                
                env.mem.write::<u32>(oldlenp, len);
                return 0;
            }
        }
    }
    
    //  CRITICAL FIX: Return -1 for everything else to prevent EA C++ asserts!
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
    
    // GLES 2.0 ENABLER: Force Gameloft/EA games to think this is an iPhone 4S
    if name_str == "hw.machine" {
        //  SILENCED TO FIX ANDROID LAG
        // log!("GAMELOFT BYPASS: Forcing hw.machine to iPhone4,1 (Enables GLES 2.0)");
        
        let hw = b"iPhone4,1\0";
        if !oldp.is_null() && !oldlenp.is_null() {
            let oldlen = env.mem.read::<u32, false>(oldlenp.cast_const());
            if oldlen as usize >= hw.len() {
                env.mem.bytes_at_mut(oldp.cast(), hw.len() as u32).copy_from_slice(hw);
            }
            env.mem.write(oldlenp, hw.len() as u32);
        }
        return 0;
    }

    //  SILENCED TO FIX ANDROID LAG
    // log!("GAMELOFT BYPASS: sysctlbyname '{}', faking hardware success", name_str);
    0
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
