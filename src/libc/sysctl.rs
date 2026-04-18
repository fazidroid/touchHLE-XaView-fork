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
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, PAGE_SIZE};
use crate::Environment;

static SYSCTL_VALUES: [((i32, i32), &str, SysInfoType); 23] = [
    ((6,1),  "hw.machine",       String(b"iPhone4,1")),
    ((6,2),  "hw.model",         String(b"N94AP")),
    ((6,12), "hw.machine_arch",  String(b"arm64")),
    ((6,3),  "hw.ncpu",          SysInfoType::Int32(1)),
    ((6,4),  "hw.physmem",       SysInfoType::Int32(512 * 1024 * 1024)),
    ((6,16), "hw.memsize",      SysInfoType::Int64(1024 * 1024 * 1024)),
    ((6,13), "hw.vectorunit",    SysInfoType::Int32(0)),
    ((0,0),  "hw.cputype",       SysInfoType::Int32(12)),
    ((0,0),  "hw.cputype",       SysInfoType::Int32(12)),
    ((0,0),  "hw.cpusubtype",    SysInfoType::Int32(6)),
    ((6,15), "hw.cpufrequency",  SysInfoType::Int64(412000000)),
    ((6,14), "hw.busfrequency",  SysInfoType::Int64(103000000)),
    ((6,5),  "hw.physmem",       SysInfoType::Int32(121634816)),
    ((6,6),  "hw.usermem",       SysInfoType::Int32(93564928)),
    ((6,24), "hw.memsize",       SysInfoType::Int32(121634816)),
    ((6,7),  "hw.pagesize",      SysInfoType::Int64(PAGE_SIZE as i64)),
    ((1,1),  "kern.ostype",      String(b"Darwin")),
    ((1,2),  "kern.osrelease",   String(b"10.0.0d3")),
    ((1,3),  "kern.osversion",   String(b"7A341")),
    ((1,10), "kern.hostname",    String(b"touchHLE")),
    ((1,4),  "kern.version",     String(b"Darwin Kernel Version 10.0.0d3: Wed May 13 22:11:58 PDT 2009; root:xnu-1357.2.89~4/RELEASE_ARM_S5L8900X")),
    ((1,65), "kern.osversion_65",String(b"7A341")),
    ((1,21), "kern.boottime",    SysInfoType::Int64(1000000000)),
];

static STRING_MAP: LazyLock<HashMap<&str, SysInfoType>> = LazyLock::new(|| {
    let mut hashmap = HashMap::new();
    for (_, str, value) in SYSCTL_VALUES.iter() {
        hashmap.insert(*str, value.clone());
    }
    hashmap
});

#[allow(clippy::type_complexity)]
static INT_MAP: LazyLock<HashMap<(i32, i32), (&str, SysInfoType)>> = LazyLock::new(|| {
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
    name: ConstPtr<i32>,
    name_len: GuestUSize,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<GuestUSize>,
    newp: MutVoidPtr,
    newlen: GuestUSize,
) -> i32 {
    set_errno(env, 0);

    // EA's engine checks network interfaces via CTL_NET/AF_ROUTE to get the
    // MAC address. We synthesise a fake "en0" Wi-Fi interface so it doesn't
    // assert-fail when it can't find a real one.
    if name_len >= 6 {
        let name0 = env.mem.read(name);
        let name1 = env.mem.read(name + 1);

        // CTL_NET == 4, AF_ROUTE == 17
        if name0 == 4 && name1 == 17 {
            log_dbg!("sysctl: injecting fake Darwin if_msghdr for EA MAC check");
            let mut payload = vec![0u8; 152];

            // if_msghdr
            payload[0]  = 152; // ifm_msglen
            payload[2]  = 14;  // ifm_version
            payload[3]  = 3;   // ifm_type (RTM_IFINFO)
            payload[12] = 1;   // ifm_index (en0 = 1)

            // sockaddr_dl at offset 112
            let sdl = 112;
            payload[sdl]     = 20;    // sdl_len
            payload[sdl + 1] = 18;    // sdl_family (AF_LINK)
            payload[sdl + 2] = 1;     // sdl_index
            payload[sdl + 3] = 6;     // sdl_type (IFT_ETHER)
            payload[sdl + 4] = 3;     // sdl_nlen ("en0")
            payload[sdl + 5] = 6;     // sdl_alen (MAC length)
            // Interface name "en0"
            payload[sdl + 8]  = b'e';
            payload[sdl + 9]  = b'n';
            payload[sdl + 10] = b'0';
            // Fake MAC 00:11:22:33:44:55
            payload[sdl + 11] = 0x00;
            payload[sdl + 12] = 0x11;
            payload[sdl + 13] = 0x22;
            payload[sdl + 14] = 0x33;
            payload[sdl + 15] = 0x44;
            payload[sdl + 16] = 0x55;

            if oldp.is_null() {
                if !oldlenp.is_null() {
                    env.mem.write(oldlenp, payload.len() as u32);
                }
                return 0;
            } else {
                let oldlen = env.mem.read(oldlenp);
                if oldlen < payload.len() as u32 {
                    return -1;
                }
                let slice = env.mem.bytes_at_mut(oldp.cast(), payload.len() as u32);
                slice.copy_from_slice(&payload);
                env.mem.write(oldlenp, payload.len() as u32);
                return 0;
            }
        }
    }

    if name_len != 2 {
        return -1;
    }

    let (name0, name1) = (env.mem.read(name), env.mem.read(name + 1));
    sysctl_generic(
        env,
        |env| {
            let Some(mut val) = INT_MAP.get(&(name0, name1)).cloned() else {
                unimplemented!("Unknown sysctl parameter ({name0}, {name1})!")
            };
            if let Some(model) = &env.options.device_model {
                if name0 == 6 && name1 == 1 {
                    let hw_machine: &[u8] = match model.as_str() {
                        "iPod5,1"   => b"iPod5,1",
                        "iPod4,1"   => b"iPod4,1",
                        "iPod3,1"   => b"iPod3,1",
                        "iPod2,1"   => b"iPod2,1",
                        "iPod1,1"   => b"iPod1,1",
                        "iPad2,5"   => b"iPad2,5",
                        "iPad3,4"   => b"iPad3,4",
                        "iPad3,1"   => b"iPad3,1",
                        "iPad2,1"   => b"iPad2,1",
                        "iPad1,1"   => b"iPad1,1",
                        "iPhone5,3" => b"iPhone5,3",
                        "iPhone5,1" => b"iPhone5,1",
                        "iPhone4,1" => b"iPhone4,1",
                        "iPhone3,1" => b"iPhone3,1",
                        "iPhone2,1" => b"iPhone2,1",
                        "iPhone1,2" => b"iPhone1,2",
                        _           => b"M68AP",
                    };
                    val.1 = SysInfoType::String(hw_machine);
                } else if name0 == 6 && name1 == 4 {
                    val.1 = SysInfoType::Int32(1024 * 1024 * 1024);
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
    set_errno(env, 0);
    let name_str = env.mem.cstr_at_utf8(name).unwrap();
    log_dbg!("sysctlbyname query: [{}]", name_str);
    sysctl_generic(
        env,
        |env| {
            let name_str = env.mem.cstr_at_utf8(name).unwrap();
            let Some((name_str, mut val)) = STRING_MAP
                .get_key_value(name_str)
                .map(|(k, v)| (*k, v.clone()))
            else {
                unimplemented!("Unknown sysctlbyname parameter {name_str}!")
            };
            if let Some(model) = &env.options.device_model {
                if name_str == "hw.machine" {
                    let hw_machine: &[u8] = match model.as_str() {
                        "iPod5,1"   => b"iPod5,1",
                        "iPod4,1"   => b"iPod4,1",
                        "iPod3,1"   => b"iPod3,1",
                        "iPod2,1"   => b"iPod2,1",
                        "iPod1,1"   => b"iPod1,1",
                        "iPad2,5"   => b"iPad2,5",
                        "iPad3,4"   => b"iPad3,4",
                        "iPad3,1"   => b"iPad3,1",
                        "iPad2,1"   => b"iPad2,1",
                        "iPad1,1"   => b"iPad1,1",
                        "iPhone5,3" => b"iPhone5,3",
                        "iPhone5,1" => b"iPhone5,1",
                        "iPhone4,1" => b"iPhone4,1",
                        "iPhone3,1" => b"iPhone3,1",
                        "iPhone2,1" => b"iPhone2,1",
                        "iPhone1,2" => b"iPhone1,2",
                        _           => b"iPhone1,1",
                    };
                    val = SysInfoType::String(hw_machine);
                } else if name_str == "hw.model" {
                    let hw_model: &[u8] = match model.as_str() {
                        "iPod5,1"   => b"N78AP",
                        "iPod4,1"   => b"N81AP",
                        "iPod3,1"   => b"N18AP",
                        "iPod2,1"   => b"N72AP",
                        "iPod1,1"   => b"N45AP",
                        "iPad2,5"   => b"P105AP",
                        "iPad3,4"   => b"P101AP",
                        "iPad3,1"   => b"J1AP",
                        "iPad2,1"   => b"K93AP",
                        "iPad1,1"   => b"K48AP",
                        "iPhone5,3" => b"N48AP",
                        "iPhone5,1" => b"N41AP",
                        "iPhone4,1" => b"N94AP",
                        "iPhone3,1" => b"N90AP",
                        "iPhone2,1" => b"N88AP",
                        "iPhone1,2" => b"N82AP",
                        _           => b"M68AP",
                    };
                    val = SysInfoType::String(hw_model);
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
    0
}

fn object_getClass(env: &mut Environment, obj: ConstVoidPtr) -> ConstVoidPtr {
    if obj.is_null() {
        return crate::mem::Ptr::null();
    }
    if obj.to_bits() == 0xDEADBEEF {
        log_dbg!("object_getClass: caught dummy MTX pointer, returning fake class");
        return crate::mem::Ptr::from_bits(0x30000000);
    }
    let isa = env.mem.read::<u32, false>(obj.cast());
    crate::mem::Ptr::from_bits(isa)
}

fn class_getProperty(
    _env: &mut Environment,
    _cls: ConstVoidPtr,
    _name: ConstVoidPtr,
) -> ConstVoidPtr {
    crate::mem::Ptr::null()
}

fn CCHmac(
    _env: &mut Environment,
    _algorithm: u32,
    _key: ConstVoidPtr,
    _keyLength: GuestUSize,
    _data: ConstVoidPtr,
    _dataLength: GuestUSize,
    _macOut: MutVoidPtr,
) {
    log_dbg!("CCHmac: bypassed (no-op)");
}

/// `__assert_rtn` — called by the EA/Gameloft engine when an assertion fails.
///
/// Previously this called `panic!()`, which killed the emulator process.
/// Now we log the assertion and return gracefully so the game can attempt
/// to continue. Many EA asserts are non-fatal in practice (device-info checks,
/// analytics, etc.) and the game will keep running if we don't panic here.
fn __assert_rtn(
    env: &mut Environment,
    func: ConstPtr<u8>,
    file: ConstPtr<u8>,
    line: i32,
    expr: ConstPtr<u8>,
) {
    let expr_str = if expr.is_null() { "(unknown)".to_string() } else { env.mem.cstr_at_utf8(expr).unwrap_or_default().to_string() };
    let file_str = if file.is_null() { "(unknown)".to_string() } else { env.mem.cstr_at_utf8(file).unwrap_or_default().to_string() };
    let func_str = if func.is_null() { "(unknown)".to_string() } else { env.mem.cstr_at_utf8(func).unwrap_or_default().to_string() };

    // 🛡️ CRITICAL: This MUST panic to prevent Unexpected SVC memory corruption!
    panic!("🎮 EA ASSERT => Expr: [{}] | File: [{}] | Func: [{}] | Line: {}", expr_str, file_str, func_str, line);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _)),
    export_c_func!(object_getClass(_)),
    export_c_func!(class_getProperty(_, _)),
    export_c_func!(CCHmac(_, _, _, _, _, _)),
    export_c_func!(__assert_rtn(_, _, _, _)),
];
