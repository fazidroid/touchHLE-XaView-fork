/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `netdb.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::sys::socket::{sockaddr, AF_INET, SOCK_DGRAM, SOCK_STREAM};
use crate::mem::{guest_size_of, ConstPtr, MutPtr, Ptr, SafeRead};
use crate::Environment;

const AI_PASSIVE: i32 = 0x1;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;

#[allow(non_camel_case_types)]
pub type socklen_t = u32;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
pub struct hostent {
    h_name: MutPtr<u8>,
    h_aliases: MutPtr<MutPtr<u8>>,
    h_addrtype: i32,
    h_length: i32,
    h_addr_list: MutPtr<MutPtr<u8>>,
}
unsafe impl SafeRead for hostent {}

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

fn getaddrinfo(
    env: &mut Environment,
    _node_name: MutPtr<u8>,
    serv_name: MutPtr<u8>,
    hints: ConstPtr<addrinfo>,
    res: MutPtr<MutPtr<addrinfo>>,
) -> i32 {
    let hint = if hints.is_null() {
        addrinfo {
            ai_flags: 0,
            ai_family: AF_INET,
            ai_socktype: SOCK_STREAM,
            ai_protocol: 0,
            ai_addrlen: 0,
            ai_canonname: Ptr::null(),
            ai_addr: Ptr::null(),
            ai_next: Ptr::null(),
        }
    } else {
        env.mem.read(hints)
    };

    let mut addr_info = hint;
    
    let port_str = env.mem.cstr_at_utf8(serv_name).unwrap_or("80");
    let port: u16 = port_str.parse().unwrap_or(80);
    
    // 🏎️ ASPHALT 8 BYPASS: Return 127.0.0.1 (Loopback)
    let addr = sockaddr::from_ipv4_parts([127, 0, 0, 1], port);

    let tmp_addr = env.mem.alloc_and_write(addr);
    addr_info.ai_addr = tmp_addr;
    addr_info.ai_addrlen = guest_size_of::<sockaddr>();

    let tmp_addr_info = env.mem.alloc_and_write(addr_info);
    env.mem.write(res, tmp_addr_info);

    0 // 0 = Success
}

fn freeaddrinfo(env: &mut Environment, addrinfo: MutPtr<addrinfo>) {
    if addrinfo.is_null() { return; }
    let addrinfo_val = env.mem.read(addrinfo);
    if !addrinfo_val.ai_addr.is_null() {
        env.mem.free(addrinfo_val.ai_addr.cast());
    }
    env.mem.free(addrinfo.cast());
}

fn gethostbyname(_env: &mut Environment, _name: ConstPtr<u8>) -> MutPtr<hostent> {
    Ptr::null()
}

// 🏎️ NEW: Missing Socket Functions Bypass
fn getpeername(env: &mut Environment, _sockfd: i32, _addr: MutPtr<sockaddr>, _addrlen: MutPtr<socklen_t>) -> i32 {
    log!("🏎️ ASPHALT 8 BYPASS: Stubbed getpeername to prevent crash!");
    crate::libc::errno::set_errno(env, 57); // ENOTCONN (Socket is not connected)
    -1 
}

fn getsockname(env: &mut Environment, _sockfd: i32, _addr: MutPtr<sockaddr>, _addrlen: MutPtr<socklen_t>) -> i32 {
    log!("🏎️ ASPHALT 8 BYPASS: Stubbed getsockname to prevent crash!");
    crate::libc::errno::set_errno(env, 57); // ENOTCONN
    -1 
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
    export_c_func!(getpeername(_, _, _)),
    export_c_func!(getsockname(_, _, _)),
];
