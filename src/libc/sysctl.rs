/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `sys/sysctl.h`

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::set_errno;
use crate::libc::sysctl::SysInfoType::String;
use crate::mem::{guest_size_of, ConstPtr, GuestUSize, MutPtr, MutVoidPtr, PAGE_SIZE};
use crate::Environment;

// Clippy complains about the type.
// Below values corresponds to the original iPhone.
// Reference https://www.mail-archive.com/misc@openbsd.org/msg80988.html
// Numerical values are from xnu/bsd/sys/sysctl.h
static SYSCTL_VALUES: [((i32, i32), &str, SysInfoType); 18] = [
    // Generic CPU, I/O
    ((6,1), "hw.machine" , String(b"iPhone1,1")),
    ((6,2), "hw.model" , String(b"M68AP")),
    ((6,3), "hw.ncpu" , SysInfoType::Int32(1)),
    ((0,0), "hw.cputype" , SysInfoType::Int32(12)),
    ((0,0), "hw.cpusubtype" , SysInfoType::Int32(6)),
    ((6,15), "hw.cpufrequency" , SysInfoType::Int64(412000000)),
    ((6,14), "hw.busfrequency" , SysInfoType::Int64(103000000)),
    ((6,5), "hw.physmem" , SysInfoType::Int32(121634816)), // not sure about this type
    ((6,6), "hw.usermem" , SysInfoType::Int32(93564928)), // not sure about this type
    ((6,24), "hw.memsize" , SysInfoType::Int32(121634816)),
    ((6,7), "hw.pagesize" , SysInfoType::Int64(PAGE_SIZE as i64)),
    // High kernel limits
    ((1,1), "kern.ostype" , String(b"Darwin")),
    ((1,2), "kern.osrelease" , String(b"10.0.0d3")),
    ((1,3), "kern.osversion" , String(b"7A341")),
    ((1,10), "kern.hostname" , String(b"touchHLE")), // this is arbitrary
    ((1,4), "kern.version" , String(b"Darwin Kernel Version 10.0.0d3: Wed May 13 22:11:58 PDT 2009; root:xnu-1357.2.89~4/RELEASE_ARM_S5L8900X")),
    ((1,65), "kern.osversion_65" , String(b"7A341")), // FakeKernOsVersion65
    ((1,21), "kern.boottime" , SysInfoType::Int64(1000000000)), // FakeBootTime
];

static STRING_MAP: LazyLock<HashMap<&str, SysInfoType>> = LazyLock::new(|| {
    // Can't use from_iter because the closure erases the lifetime
    let mut hashmap = HashMap::new();
    for (_, str, value) in SYSCTL_VALUES.iter() {
        hashmap.insert(*str, value.clone());
    }
    hashmap
});

#[allow(clippy::type_complexity)]
static INT_MAP: LazyLock<HashMap<(i32, i32), (&str, SysInfoType)>> = LazyLock::new(|| {
    // Can't use from_iter because the closure erases the lifetime
    let mut hashmap = HashMap::new();
    for (ints, str, value) in SYSCTL_VALUES.iter() {
        hashmap.insert(*ints, (*str, value.clone()));
    }
    hashmap
});

#[derive(Clone)]
enum SysInfoType {
    String(&'static [u8]),
    Int32(i32),
    Int64(i64),
}

fn sysctl(
    env: &mut Environment,
    name: MutPtr<i32>,
    name_len: u32,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<GuestUSize>,
    newp: MutVoidPtr,
    newlen: GuestUSize,
) -> i32 {
    set_errno(env, 0);

    log_dbg!(
        "sysctl({:?}, {:#x}, {:?}, {:?}, {:?}, {:x})",
        name,
        name_len,
        oldp,
        oldlenp,
        newp,
        newlen
    );

    // --- GAMELOFT MAC ADDRESS BYPASS START ---
    if name_len == 6 {
        let mut mib = [0i32; 6];
        for i in 0..6 {
            mib[i] = env.mem.read(name + (i as u32));
        }

        // Check for CTL_NET (4), AF_ROUTE (17), AF_LINK (18), NET_RT_IFLIST (3)
        if mib[0] == 4 && mib[1] == 17 && mib[3] == 18 && mib[4] == 3 {
            log!("sysctl: Faking MAC address response for Gameloft FederationManager");
            
            let fake_size = 152u32; 
            
            if !oldlenp.is_null() {
                env.mem.write(oldlenp, fake_size);
            }
            
            if !oldp.is_null() {
                let oldlen = env.mem.read(oldlenp);
                if oldlen < fake_size {
                    log!("sysctl MAC bypass: buffer too small ({} < {})", oldlen, fake_size);
                    return -1;
                }

                // Safely cast to a u8 pointer for byte writing
                let oldp_u8 = oldp.cast::<u8>();

                // 1. Zero memory
                for i in 0..fake_size {
                    env.mem.write(oldp_u8 + i, 0u8);
                }
                
                // 2. if_msghdr
                env.mem.write(oldp_u8 + 0u32, 152u8); // ifm_msglen
                env.mem.write(oldp_u8 + 1u32, 0u8);
                env.mem.write(oldp_u8 + 2u32, 5u8);   // ifm_version
                env.mem.write(oldp_u8 + 3u32, 14u8);  // ifm_type
                env.mem.write(oldp_u8 + 12u32, 1u8);  // ifm_index
                
                // 3. sockaddr_dl
                // 3. sockaddr_dl
                let sdl_offset = 112u32; // FIXED: 32-bit iOS if_msghdr size is 112 bytes
                env.mem.write(oldp_u8 + sdl_offset + 0u32, 20u8); // sdl_len
                env.mem.write(oldp_u8 + sdl_offset + 1u32, 18u8); // sdl_family
                env.mem.write(oldp_u8 + sdl_offset + 2u32, 1u8);  // sdl_index
                env.mem.write(oldp_u8 + sdl_offset + 4u32, 6u8);  // sdl_type
                env.mem.write(oldp_u8 + sdl_offset + 5u32, 3u8);  // sdl_nlen
                env.mem.write(oldp_u8 + sdl_offset + 6u32, 6u8);  // sdl_alen
                
                env.mem.write(oldp_u8 + sdl_offset + 8u32, b'e');
                env.mem.write(oldp_u8 + sdl_offset + 9u32, b'n');
                env.mem.write(oldp_u8 + sdl_offset + 10u32, b'0');
                
                // Real MAC address to prevent (null) UDID hash errors!
                env.mem.write(oldp_u8 + sdl_offset + 11u32, 0x12u8);
                env.mem.write(oldp_u8 + sdl_offset + 12u32, 0x34u8);
                env.mem.write(oldp_u8 + sdl_offset + 13u32, 0x56u8);
                env.mem.write(oldp_u8 + sdl_offset + 14u32, 0x78u8);
                env.mem.write(oldp_u8 + sdl_offset + 15u32, 0x9Au8);
                env.mem.write(oldp_u8 + sdl_offset + 16u32, 0xBCu8);
            }
            return 0;
        }
    }
    // --- GAMELOFT MAC ADDRESS BYPASS END ---

    if name_len != 2 {
        log!(
            "TODO: sysctl called with name_len = {} (expected 2). Faking empty response to avoid crash.",
            name_len
        );
        // Если игра запрашивает размер данных
        if !oldlenp.is_null() {
            env.mem.write(oldlenp, 0);
        }
        // ОБЯЗАТЕЛЬНО возвращаем 0 (успех)
        return 0;
    }

    let (name0, name1) = (env.mem.read(name), env.mem.read(name + 1));
    sysctl_generic(
        env,
        |env| {
            // MutateEnvCapture
            let Some(mut val) = INT_MAP.get(&(name0, name1)).cloned() else {
                unimplemented!("Unknown sysctl parameter ({name0}, {name1})!")
            };
            if let Some(model) = &env.options.device_model {
                // CheckModelOverride
                if name0 == 6 && name1 == 1 {
                    let hw_machine: &[u8] = match model.as_str() {
                        // MatchHwMachine
                        "iPod5,1" => b"iPod5,1",
                        "iPod4,1" => b"iPod4,1",
                        "iPod3,1" => b"iPod3,1",
                        "iPod2,1" => b"iPod2,1",
                        "iPod1,1" => b"iPod1,1",
                        "iPad2,5" => b"iPad2,5",
                        "iPad3,4" => b"iPad3,4",
                        "iPad3,1" => b"iPad3,1",
                        "iPad2,1" => b"iPad2,1",
                        "iPad1,1" => b"iPad1,1",
                        "iPhone5,3" => b"iPhone5,3",
                        "iPhone5,1" => b"iPhone5,1",
                        "iPhone4,1" => b"iPhone4,1",
                        "iPhone3,1" => b"iPhone3,1",
                        "iPhone2,1" => b"iPhone2,1",
                        "iPhone1,2" => b"iPhone1,2",
                        _ => b"iPhone1,1",
                    };
                    val.1 = SysInfoType::String(hw_machine); // OverrideMachine
                } else if name0 == 6 && name1 == 2 {
                    let hw_model: &[u8] = match model.as_str() {
                        // MatchHwModel
                        "iPod5,1" => b"N78AP",
                        "iPod4,1" => b"N81AP",
                        "iPod3,1" => b"N18AP",
                        "iPod2,1" => b"N72AP",
                        "iPod1,1" => b"N45AP",
                        "iPad2,5" => b"P105AP",
                        "iPad3,4" => b"P101AP",
                        "iPad3,1" => b"J1AP",
                        "iPad2,1" => b"K93AP",
                        "iPad1,1" => b"K48AP",
                        "iPhone5,3" => b"N48AP",
                        "iPhone5,1" => b"N41AP",
                        "iPhone4,1" => b"N94AP",
                        "iPhone3,1" => b"N90AP",
                        "iPhone2,1" => b"N88AP",
                        "iPhone1,2" => b"N82AP",
                        _ => b"M68AP",
                    };
                    val.1 = SysInfoType::String(hw_model); // OverrideModel
                }
            }
            val
        },
        oldp,
        oldlenp,
        newp,
        newlen,
    )
}

fn sysctlbyname(
    env: &mut Environment,
    name: ConstPtr<u8>,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<GuestUSize>,
    newp: MutVoidPtr,
    newlen: GuestUSize,
) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    let name_str = env.mem.cstr_at_utf8(name).unwrap();
    log_dbg!(
        "sysctlbyname({:?}, {:?}, {:?}, {:?}, {:x})",
        name_str,
        oldp,
        oldlenp,
        newp,
        newlen
    );
    sysctl_generic(
        env,
        |env| {
            // MutateEnvCapture
            let name_str = env.mem.cstr_at_utf8(name).unwrap();
            let Some((name_str, mut val)) = STRING_MAP
                .get_key_value(name_str)
                .map(|(k, v)| (*k, v.clone()))
            else {
                unimplemented!("Unknown sysctlbyname parameter {name_str}!")
            };
            if let Some(model) = &env.options.device_model {
                // CheckModelOverride
                if name_str == "hw.machine" {
                    let hw_machine: &[u8] = match model.as_str() {
                        // MatchHwMachine
                        "iPod5,1" => b"iPod5,1",
                        "iPod4,1" => b"iPod4,1",
                        "iPod3,1" => b"iPod3,1",
                        "iPod2,1" => b"iPod2,1",
                        "iPod1,1" => b"iPod1,1",
                        "iPad2,5" => b"iPad2,5",
                        "iPad3,4" => b"iPad3,4",
                        "iPad3,1" => b"iPad3,1",
                        "iPad2,1" => b"iPad2,1",
                        "iPad1,1" => b"iPad1,1",
                        "iPhone5,3" => b"iPhone5,3",
                        "iPhone5,1" => b"iPhone5,1",
                        "iPhone4,1" => b"iPhone4,1",
                        "iPhone3,1" => b"iPhone3,1",
                        "iPhone2,1" => b"iPhone2,1",
                        "iPhone1,2" => b"iPhone1,2",
                        _ => b"iPhone1,1",
                    };
                    val = SysInfoType::String(hw_machine); // OverrideMachine
                } else if name_str == "hw.model" {
                    let hw_model: &[u8] = match model.as_str() {
                        // MatchHwModel
                        "iPod5,1" => b"N78AP",
                        "iPod4,1" => b"N81AP",
                        "iPod3,1" => b"N18AP",
                        "iPod2,1" => b"N72AP",
                        "iPod1,1" => b"N45AP",
                        "iPad2,5" => b"P105AP",
                        "iPad3,4" => b"P101AP",
                        "iPad3,1" => b"J1AP",
                        "iPad2,1" => b"K93AP",
                        "iPad1,1" => b"K48AP",
                        "iPhone5,3" => b"N48AP",
                        "iPhone5,1" => b"N41AP",
                        "iPhone4,1" => b"N94AP",
                        "iPhone3,1" => b"N90AP",
                        "iPhone2,1" => b"N88AP",
                        "iPhone1,2" => b"N82AP",
                        _ => b"M68AP",
                    };
                    val = SysInfoType::String(hw_model); // OverrideModel
                }
            }
            (name_str, val)
        },
        oldp,
        oldlenp,
        newp,
        newlen,
    )
}

fn sysctl_generic<F>(
    env: &mut Environment,
    // Returns the name and value of the property (or exits)
    name_lookup: F,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<GuestUSize>,
    newp: MutVoidPtr,
    newlen: GuestUSize,
) -> i32
where
    F: FnOnce(&mut Environment) -> (&'static str, SysInfoType),
{
    assert!(newp.is_null());
    assert_eq!(newlen, 0);

    let (name_str, val) = name_lookup(env);
    let len: GuestUSize = match val {
        String(str) => str.len() as GuestUSize + 1,
        SysInfoType::Int32(_) => guest_size_of::<i32>(),
        SysInfoType::Int64(_) => guest_size_of::<i64>(),
    };
    if oldp.is_null() {
        env.mem.write(oldlenp, len);
        return 0;
    }
    assert!(!oldp.is_null() && !oldlenp.is_null());
    let oldlen = env.mem.read(oldlenp);
    if oldlen < len {
        // TODO: set errno
        // TODO: write partial data
        log!("sysctl(byname) for '{name_str}': the buffer of size {oldlen} is too low to fit the value of size {len}, returning -1");
        return -1;
    }
    match val {
        String(str) => {
            let sysctl_str = env.mem.alloc_and_write_cstr(str);
            env.mem.memmove(oldp, sysctl_str.cast().cast_const(), len);
            env.mem.free(sysctl_str.cast());
        }
        SysInfoType::Int32(num) => {
            env.mem.write(oldp.cast(), num);
        }
        SysInfoType::Int64(num) => {
            env.mem.write(oldp.cast(), num);
        }
    }
    env.mem.write(oldlenp, len);
    0 // success
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _)),
];
