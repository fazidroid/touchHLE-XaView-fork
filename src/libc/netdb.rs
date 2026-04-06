/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `netdb.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::sys::socket::{sockaddr, AF_INET, SOCK_DGRAM, SOCK_STREAM};
use crate::mem::{guest_size_of, ConstPtr, MutPtr, SafeRead};
use crate::Environment;
use super::net_bypass::NetBypass; // Our bypass manager

const AI_PASSIVE: i32 = 0x1;

pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;

const EAI_FAIL: i32 = 4;
const EAI_NONAME: i32 = 8; // EAI_NONAME: Error code for "Host not found"

#[allow(non_camel_case_types)]
pub type socklen_t = u32;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
pub struct addrinfo {
    ai_flags: i32,
    ai_family: i32,
    ai_socktype: i32,
    ai_protocol: i32,
    ai_addrlen: socklen_t,
    ai_canonname: MutPtr<u8>,
    ai_addr: MutPtr<sockaddr>,
    ai_next: MutPtr<addrinfo>,
}
unsafe impl SafeRead for addrinfo {}

// ===== HOSTENT STRUCT REWRITTEN TO SPOOF SERVERS =====
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
pub struct hostent {
    h_name: MutPtr<u8>,
    h_aliases: MutPtr<u32>,
    h_addrtype: i32,
    h_length: i32,
    h_addr_list: MutPtr<u32>,
}
unsafe impl SafeRead for hostent {}

// Custom safe wrapper structs to write the DNS payload
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct FakeIP { b1: u8, b2: u8, b3: u8, b4: u8 }
unsafe impl SafeRead for FakeIP {}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct FakeAddrList { ptr: u32, null_term: u32 }
unsafe impl SafeRead for FakeAddrList {}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct FakeAliasList { null_term: u32 }
unsafe impl SafeRead for FakeAliasList {}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
];

fn getaddrinfo(
    env: &mut Environment,
    nodename: ConstPtr<u8>,
    servname: ConstPtr<u8>,
    hints: ConstPtr<addrinfo>,
    res: MutPtr<MutPtr<addrinfo>>,
) -> i32 {
    let nodename_str = if !nodename.is_null() {
        env.mem.cstr_at_utf8(nodename).unwrap_or("unknown")
    } else {
        "unknown"
    };

    // --- ASPHALT 6 GETADDRINFO BYPASS START ---
    // This catches the mid-game LoadProfile config checks!
    if NetBypass::is_blocked_domain(nodename_str) {
        log!("Bypass: Blackholing getaddrinfo request for dead server: {}", nodename_str);
        return EAI_NONAME; // Return host not found immediately
    }
    // --- ASPHALT 6 GETADDRINFO BYPASS END ---

    log!(
        "Spoofing DNS request for getaddrinfo({:?} \"{}\") => 127.0.0.1",
        nodename,
        nodename_str
    );

    let mut out_addrinfo = addrinfo {
        ai_flags: 0,
        ai_family: AF_INET,
        ai_socktype: SOCK_STREAM,
        ai_protocol: IPPROTO_TCP,
        ai_addrlen: 16, // sizeof(sockaddr_in)
        ai_canonname: MutPtr::null(),
        ai_addr: MutPtr::null(),
        ai_next: MutPtr::null(),
    };

    if !hints.is_null() {
        let hints_val = env.mem.read(hints);
        if hints_val.ai_flags & AI_PASSIVE != 0 {
            // Passive not strictly handled for spoofing
        }
        if hints_val.ai_family != 0 {
            out_addrinfo.ai_family = hints_val.ai_family;
        }
        if hints_val.ai_socktype != 0 {
            out_addrinfo.ai_socktype = hints_val.ai_socktype;
        }
        if hints_val.ai_protocol != 0 {
            out_addrinfo.ai_protocol = hints_val.ai_protocol;
        }
    }

    // Spoof to 127.0.0.1
    let mut ip_addr = [0u8; 16]; 
    ip_addr[0] = 16; // sin_len
    ip_addr[1] = AF_INET as u8; // sin_family
    ip_addr[4] = 127; // sin_addr
    ip_addr[5] = 0;
    ip_addr[6] = 0;
    ip_addr[7] = 1;

    let ip_ptr = env.mem.alloc_and_write_bytes(&ip_addr);
    out_addrinfo.ai_addr = ip_ptr.cast();

    let addrinfo_ptr = env.mem.alloc_and_write(out_addrinfo);
    env.mem.write(res, addrinfo_ptr);

    0
}

fn freeaddrinfo(env: &mut Environment, addrinfo: MutPtr<addrinfo>) {
    if addrinfo.is_null() {
        return;
    }
    let addrinfo_val = env.mem.read(addrinfo);
    // Free the inner sockaddr memory block we allocated
    env.mem.free(addrinfo_val.ai_addr.cast());
    env.mem.free(addrinfo.cast());
}

fn gethostbyname(env: &mut Environment, name: ConstPtr<u8>) -> MutPtr<hostent> {
    let name_str = env.mem.cstr_at_utf8(name).unwrap_or("unknown");

    // --- GAMELOFT GETHOSTBYNAME BYPASS START ---
    // This catches the initial boot DRM checks
    if NetBypass::is_blocked_domain(name_str) {
        log!("Bypass: Blackholing gethostbyname request for dead server: {}", name_str);
        return MutPtr::null();
    }
    // --- GAMELOFT GETHOSTBYNAME BYPASS END ---

    log!(
        "Spoofing DNS request for gethostbyname({:?} \"{}\") => 127.0.0.1",
        name,
        name_str
    );

    // 1. Create a fake IP address (127.0.0.1 / localhost)
    let ip_addr = FakeIP { b1: 127, b2: 0, b3: 0, b4: 1 };
    let ip_ptr = env.mem.alloc_and_write(ip_addr);

    // 2. Create the null-terminated address list required by C structs
    let addr_list = FakeAddrList { ptr: ip_ptr.to_bits(), null_term: 0 };
    let addr_list_ptr = env.mem.alloc_and_write(addr_list);

    // 3. Create the null-terminated aliases list
    let aliases = FakeAliasList { null_term: 0 };
    let aliases_ptr = env.mem.alloc_and_write(aliases);

    // 4. Populate the full hostent struct so the game engine happily reads it
    let hostent_data = hostent {
        h_name: name.cast_mut(),
        h_aliases: aliases_ptr.cast(),
        h_addrtype: AF_INET,
        h_length: 4,
        h_addr_list: addr_list_ptr.cast(),
    };

    env.mem.alloc_and_write(hostent_data).cast()
}
