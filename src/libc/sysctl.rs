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
    ((0,0), "hw.cpusubtype" , SysInfoType::Int32(0)),
    ((6,4), "hw.byteorder" , SysInfoType::Int32(1234)),
    ((6,5), "hw.physmem" , SysInfoType::Int32(130023424)),
    ((6,6), "hw.usermem" , SysInfoType::Int32(104857600)),
    ((6,7), "hw.pagesize" , SysInfoType::Int32(PAGE_SIZE as i32)), // 0x1000, 4K
    ((0,0), "hw.busfreq" , SysInfoType::Int32(100000000)), // 100MHz bus
    ((0,0), "hw.cpufreq" , SysInfoType::Int32(412000000)), // 412MHz underclocked CPU
    ((0,0), "hw.cachelinesize" , SysInfoType::Int32(32)),
    ((0,0), "hw.l1icachesize" , SysInfoType::Int32(16384)), // 16KB L1I
    ((0,0), "hw.l1dcachesize" , SysInfoType::Int32(16384)), // 16KB L1D
    ((0,0), "hw.l2cachesize" , SysInfoType::Int32(0)), // original iPhone apparently has no L2?

    ((6,24), "hw.optional.floatingpoint" , SysInfoType::Int32(1)),

    // Generic system
    ((1,1), "kern.ostype" , String(b"Darwin")),
    ((1,2), "kern.osrelease" , String(b"9.0.0d1")),
];

static SYSCTL_BY_NAME: LazyLock<HashMap<&'static str, SysInfoType>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for (_mib, name, val) in SYSCTL_VALUES.iter() {
        map.insert(*name, *val);
    }
    map
});

static SYSCTL_BY_MIB: LazyLock<HashMap<(i32, i32), SysInfoType>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for (mib, _name, val) in SYSCTL_VALUES.iter() {
        map.insert(*mib, *val);
    }
    map
});

#[derive(Clone, Copy)]
enum SysInfoType {
    String(&'static [u8]),
    Int32(i32),
    Int64(i64),
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sysctl(_, _, _, _, _, _)),
    export_c_func!(sysctlbyname(_, _, _, _, _)),
];

pub fn sysctl(
    env: &mut Environment,
    name_ptr: ConstPtr<i32>,
    name_len: u32,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<GuestUSize>,
    newp: MutVoidPtr,
    newlen: GuestUSize,
) -> i32 {
    // --- GAMELOFT MAC ADDRESS BYPASS START ---
    if name_len == 6 {
        let mut mib = [0i32; 6];
        for i in 0..6 {
            // Explicitly tell Rust we are reading an i32 to fix the compiler error
            let val: i32 = env.mem.read(name_ptr.offset(i as isize)).unwrap_or(0i32);
            mib[i] = val;
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

                // 1. Zero memory
                for i in 0..fake_size {
                    env.mem.write(oldp.offset(i as isize), 0u8);
                }
                
                // 2. if_msghdr
                env.mem.write(oldp.offset(0), 152u8); // ifm_msglen
                env.mem.write(oldp.offset(1), 0u8);
                env.mem.write(oldp.offset(2), 5u8);   // ifm_version
                env.mem.write(oldp.offset(3), 14u8);  // ifm_type
                env.mem.write(oldp.offset(12), 1u8);  // ifm_index
                
                // 3. sockaddr_dl
                let sdl_offset = 76isize;
                env.mem.write(oldp.offset(sdl_offset + 0), 20u8); // sdl_len
                env.mem.write(oldp.offset(sdl_offset + 1), 18u8); // sdl_family
                env.mem.write(oldp.offset(sdl_offset + 2), 1u8);  // sdl_index
                env.mem.write(oldp.offset(sdl_offset + 4), 6u8);  // sdl_type
                env.mem.write(oldp.offset(sdl_offset + 5), 3u8);  // sdl_nlen
                env.mem.write(oldp.offset(sdl_offset + 6), 6u8);  // sdl_alen
                
                env.mem.write(oldp.offset(sdl_offset + 8), b'e');
                env.mem.write(oldp.offset(sdl_offset + 9), b'n');
                env.mem.write(oldp.offset(sdl_offset + 10), b'0');
                
                env.mem.write(oldp.offset(sdl_offset + 11), 0x00u8);
                env.mem.write(oldp.offset(sdl_offset + 12), 0x11u8);
                env.mem.write(oldp.offset(sdl_offset + 13), 0x22u8);
                env.mem.write(oldp.offset(sdl_offset + 14), 0x33u8);
                env.mem.write(oldp.offset(sdl_offset + 15), 0x44u8);
                env.mem.write(oldp.offset(sdl_offset + 16), 0x55u8);
            }
            return 0;
        }
    }
    // --- GAMELOFT MAC ADDRESS BYPASS END ---

    if name_len != 2 {
        log!("TODO: sysctl called with name_len = {} (expected 2). Faking empty response to avoid crash.", name_len);
        if !oldlenp.is_null() {
            env.mem.write(oldlenp, 0);
        }
        return 0;
    }

    let mib = (
        env.mem.read(name_ptr.offset(0)),
        env.mem.read(name_ptr.offset(1)),
    );

    sysctl_impl(env, oldp, oldlenp, newp, newlen, |env| {
        if let Some(val) = SYSCTL_BY_MIB.get(&mib) {
            let name_str = SYSCTL_VALUES.iter().find(|(m, _, _)| *m == mib).unwrap().1;
            (name_str, *val)
        } else {
            // TODO return ENOTDIR
            panic!(
                "TODO: sysctl called with unimplemented name: {:?}",
                (mib.0, mib.1)
            );
        }
    })
}

pub fn sysctlbyname(
    env: &mut Environment,
    name_ptr: ConstPtr<u8>,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<GuestUSize>,
    newp: MutVoidPtr,
    newlen: GuestUSize,
) -> i32 {
    let name_str = env.mem.cstr_at_utf8(name_ptr).unwrap();

    sysctl_impl(env, oldp, oldlenp, newp, newlen, |env| {
        if let Some(val) = SYSCTL_BY_NAME.get(name_str) {
            (name_str, *val)
        } else {
            set_errno(env, crate::libc::errno::ENOENT);
            log!("TODO: sysctlbyname called with unimplemented name: {name_str}, returning ENOENT");
            // Workaround for https://github.com/hikari-no-yume/touchHLE/issues/4
            ("not implemented", String(b"TODO!"))
        }
    })
}

fn sysctl_impl<F>(
    env: &mut Environment,
    oldp: MutVoidPtr,
    oldlenp: MutPtr<GuestUSize>,
    newp: MutVoidPtr,
    newlen: GuestUSize,
    name_lookup: F, // <--- Restored missing parameter!
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
        SysInfoType::Int32(num) => env.mem.write(oldp.cast(), num),
        SysInfoType::Int64(num) => env.mem.write(oldp.cast(), num),
    }

    env.mem.write(oldlenp, len);
    0
}
