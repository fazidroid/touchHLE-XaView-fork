/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0.
 * If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `sys/sysctl.h`

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::set_errno;
use crate::libc::sysctl::SysInfoType::String;
// 🛡️ FIXED IMPORT: Added ConstVoidPtr for object_getClass
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, PAGE_SIZE};
use crate::Environment;

// Clippy complains about the type.
// Below values corresponds to the original iPhone.
// Reference https://www.mail-archive.com/misc@openbsd.org/msg80988.html
// Numerical values are from xnu/bsd/sys/sysctl.h

// 🛡️ FIX: Updated array size from 18 to 20 to account for new Asphalt 8 additions
static SYSCTL_VALUES: [((i32, i32), &str, SysInfoType); 20] = [
    // Generic CPU, I/O
    ((6,1), "hw.machine" , String(b"iPhone1,1")),
    ((6,2), "hw.model" , String(b"M68AP")),
    ((6,3), "hw.ncpu" , SysInfoType::Int32(1)),
    ((6,4), "hw.physmem" , SysInfoType::Int32(512 * 1024 * 1024)),
    ((0,0), "hw.cputype" , SysInfoType::Int32(12)),
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

    // ==========================================================
    // 🏎️ EA BYPASS: Network Check / MAC Address Hack
    // ==========================================================
    if name_len >= 2 {
        let name0 = env.mem.read(name);
        let name1 = env.mem.read(name + 1);
        if name0 == 4 && name1 == 17 {
            if !oldlenp.is_null() {
                env.mem.write(oldlenp, 6); // Fake a 6-byte MAC address
            }
            return 0; // SUCCESS
        }
    }

    if name_len != 2 {
        return -1; // CRITICAL: This MUST return -1 (Error), not 0!
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
                        _ => b"M68AP",
                    };
                    val.1 = SysInfoType::String(hw_machine); // OverrideMachine
                                } else if name0 == 6 && name1 == 4 {
                    // 🏎️ UPGRADE: 1GB of RAM for Asphalt 8 support!
                    val.1 = SysInfoType::Int32(1024 * 1024 * 1024); // OverrideModel
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
                    val = SysInfoType::String(hw_machine);
                // OverrideMachine
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
        // 🛡️ SILENCED 60FPS LOG: Prevent Android logcat spam
        // log!("sysctl(byname) for '{name_str}': the buffer of size {oldlen} is too low to fit the value of size {len}, returning -1");
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

// ==========================================================
// 🏎️ EA BYPASS: object_getClass Dummy Memory Protection
// ==========================================================
fn object_getClass(env: &mut Environment, obj: ConstVoidPtr) -> ConstVoidPtr {
    if obj.is_null() {
        return crate::mem::Ptr::null();
    }

    // EA Bypass: If the game passes our dummy MTX pointer, 
    // return a fake valid Class pointer to prevent the engine from panicking.
    if obj.to_bits() == 0xDEADBEEF {
        println!("🎮 LOG: Caught object_getClass reading dummy MTX pointer!");
        return crate::mem::Ptr::from_bits(0x30000000); 
    }

    let isa = env.mem.read::<u32, false>(obj.cast());
    crate::mem::Ptr::from_bits(isa)
}

// ==========================================================
// 🏎️ EA BYPASS: class_getProperty Dummy Reflection
// ==========================================================
fn class_getProperty(_env: &mut Environment, _cls: ConstVoidPtr, _name: ConstVoidPtr) -> ConstVoidPtr {
    // EA's engine is checking the properties of our dummy MTX class.
    // Returning NULL safely tells it "this class has no properties," 
    // satisfying the reflection check without causing memory access violations.
    crate::mem::Ptr::null() 
}
// ==========================================================
// 🏎️ GAMELOFT BYPASS: CCHmac Dummy Crypto
// ==========================================================
fn CCHmac(
    _env: &mut Environment,
    _algorithm: u32,
    _key: ConstVoidPtr,
    _keyLength: GuestUSize,
    _data: ConstVoidPtr,
    _dataLength: GuestUSize,
    _macOut: MutVoidPtr,
) {
    println!("🎮 LOG: Bypassing Gameloft CCHmac crypto check!");
    // The game is trying to cryptographically sign an ad-network request.
    // By returning without doing any math, we safely neuter the analytics check!
}
// ==============================================
// 🏎️ EA BYPASS: __assert_rtn Catch
// ==============================================
fn __assert_rtn(
    env: &mut Environment,
    _func: crate::mem::ConstPtr<u8>,
    _file: crate::mem::ConstPtr<u8>,
    line: i32,
    expr: crate::mem::ConstPtr<u8>,
) {
    let expr_str = if expr.is_null() { 
        "(unknown)".to_string() 
    } else { 
        env.mem.cstr_at_utf8(expr).unwrap_or_default().to_string() 
    };
    
    panic!("🎮 EA Engine Assert Triggered! Expression: {} (Line {})", expr_str, line);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _)),
    export_c_func!(object_getClass(_)),
    // FIXED: Removed the extra underscore!
    export_c_func!(class_getProperty(_, _)), 
    export_c_func!(CCHmac(_, _, _, _, _, _)),
    export_c_func!(__assert_rtn(_, _, _, _)),
];
