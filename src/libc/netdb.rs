/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `netdb.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::sys::socket::{sockaddr, AF_INET, SOCK_STREAM};
use crate::mem::{guest_size_of, ConstPtr, MutPtr, SafeRead};
use crate::Environment;

const AI_PASSIVE: i32 = 0x1;

pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;

const EAI_FAIL: i32 = 4;

#[allow(non_camel_case_types)]
pub type socklen_t = u32;

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

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct FakeIP {
    b1: u8,
    b2: u8,
    b3: u8,
    b4: u8,
}
unsafe impl SafeRead for FakeIP {}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct FakeAddrList {
    ptr: u32,
    null_term: u32,
}
unsafe impl SafeRead for FakeAddrList {}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct FakeAliasList {
    null_term: u32,
}
unsafe impl SafeRead for FakeAliasList {}

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
    _node_name: MutPtr<u8>, // FIXED: Prefixed with underscore to ignore warning
    serv_name: MutPtr<u8>,
    hints: ConstPtr<addrinfo>,
    res: MutPtr<MutPtr<addrinfo>>,
) -> i32 {
    if !env.options.network_access {
        return EAI_FAIL;
    }

    let hint = if !hints.is_null() {
        env.mem.read(hints)
    } else {
        addrinfo {
            ai_flags: 0,
            ai_family: AF_INET,
            ai_socktype: SOCK_STREAM,
            ai_protocol: IPPROTO_TCP,
            ai_addrlen: 0,
            ai_canonname: MutPtr::null(),
            ai_addr: MutPtr::null(),
            ai_next: MutPtr::null(),
        }
    };

    let mut addr_info = hint;
    
    let port_str = if !serv_name.is_null() {
        env.mem.cstr_at_utf8(serv_name.cast_const()).unwrap_or("80")
    } else {
        "80"
    };
    let port: u16 = port_str.parse().unwrap_or(80);
    
    // SPOOF TO 127.0.0.1 - Triggers instant ECONNREFUSED to break retry loops
    let addr = sockaddr::from_ipv4_parts([127, 0, 0, 1], port);

    let tmp_addr = env.mem.alloc_and_write(addr);
    addr_info.ai_addr = tmp_addr;
    addr_info.ai_addrlen = guest_size_of::<sockaddr>();
    addr_info.ai_next = MutPtr::null();

    let tmp_addr_info = env.mem.alloc_and_write(addr_info);
    env.mem.write(res, tmp_addr_info);

    0 // Success
}

fn freeaddrinfo(env: &mut Environment, addrinfo: MutPtr<addrinfo>) {
    if addrinfo.is_null() {
        return;
    }
    let addrinfo_val = env.mem.read(addrinfo);
    
    let ai_addrlen = addrinfo_val.ai_addrlen;
    if ai_addrlen == guest_size_of::<sockaddr>() {
        let _ = env.mem.free(addrinfo_val.ai_addr.cast());
    }
    let _ = env.mem.free(addrinfo.cast());
}

fn gethostbyname(env: &mut Environment, name: ConstPtr<u8>) -> MutPtr<hostent> {
    let name_str = env.mem.cstr_at_utf8(name).unwrap_or("unknown");

    log!(
        "Spoofing DNS request for gethostbyname({:?} \"{}\") => 127.0.0.1 to force TCP failure",
        name,
        name_str
    );

    let ip_addr = FakeIP { b1: 127, b2: 0, b3: 0, b4: 1 };
    let ip_ptr = env.mem.alloc_and_write(ip_addr);

    let addr_list = FakeAddrList { ptr: ip_ptr.to_bits(), null_term: 0 };
    let addr_list_ptr = env.mem.alloc_and_write(addr_list);

    let aliases = FakeAliasList { null_term: 0 };
    let aliases_ptr = env.mem.alloc_and_write(aliases);

    let hostent_data = hostent {
        h_name: name.cast_mut(),
        h_aliases: aliases_ptr.cast(),
        h_addrtype: AF_INET,
        h_length: 4, 
        h_addr_list: addr_list_ptr.cast(),
    };

    env.mem.alloc_and_write(hostent_data)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
];