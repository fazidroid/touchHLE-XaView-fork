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
use super::net_bypass::NetBypass;

const AI_PASSIVE: i32 = 0x1;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;
const EAI_FAIL: i32 = 4;

#[allow(non_camel_case_types)]
pub type socklen_t = u32;

// ===== STRUCTS FOR DNS SPOOFING =====
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
// ===================================

fn getaddrinfo(
    env: &mut Environment,
    node_name: MutPtr<u8>,
    serv_name: MutPtr<u8>,
    hints: ConstPtr<addrinfo>,
    res: MutPtr<MutPtr<addrinfo>>,
) -> i32 {
    if !env.options.network_access {
        log_dbg!(
            "Network access is disabled, getaddrinfo({:?}, {:?}, {:?}, {:?}) -> EAI_FAIL",
            node_name,
            serv_name,
            hints,
            res
        );
        return EAI_FAIL;
    }

    assert!(node_name.is_null()); // TODO

    let hint = env.mem.read(hints);
    let ai_flags = hint.ai_flags;
    assert_eq!(ai_flags, AI_PASSIVE);
    let ai_family = hint.ai_family;
    assert_eq!(ai_family, AF_INET);
    assert!(hint.ai_socktype == SOCK_STREAM || hint.ai_socktype == SOCK_DGRAM);
    assert!(
        hint.ai_protocol == IPPROTO_TCP || hint.ai_protocol == IPPROTO_UDP || hint.ai_protocol == 0
    );
    let ai_addrlen = hint.ai_addrlen;
    assert_eq!(ai_addrlen, 0);
    assert!(hint.ai_canonname.is_null());
    assert!(hint.ai_addr.is_null());
    assert!(hint.ai_next.is_null());

    let mut addr_info = hint;
    let port: u16 = env.mem.cstr_at_utf8(serv_name).unwrap().parse().unwrap();
    log_dbg!("getaddrinfo: port {}", port);
    let addr = sockaddr::from_ipv4_parts([0; 4], port);

    let tmp_addr = env.mem.alloc_and_write(addr);
    addr_info.ai_addr = tmp_addr;
    addr_info.ai_addrlen = guest_size_of::<sockaddr>();

    let tmp_addr_info = env.mem.alloc_and_write(addr_info);
    env.mem.write(res, tmp_addr_info);

    0 // Success
}

fn freeaddrinfo(env: &mut Environment, addrinfo: MutPtr<addrinfo>) {
    let addrinfo_val = env.mem.read(addrinfo);
    assert!(addrinfo_val.ai_next.is_null()); // TODO
    let ai_addrlen = addrinfo_val.ai_addrlen;
    assert_eq!(ai_addrlen, guest_size_of::<sockaddr>());
    env.mem.free(addrinfo_val.ai_addr.cast());
    env.mem.free(addrinfo.cast());
}

fn gethostbyname(env: &mut Environment, name: ConstPtr<u8>) -> MutPtr<hostent> {
    let name_str = env.mem.cstr_at_utf8(name).unwrap_or("unknown");

    // 1. BYPASS CHECK: Block dead servers immediately
    if NetBypass::is_blocked_domain(name_str) {
        log!("Bypass: Blackholing DNS request for dead server: {}", name_str);
        return MutPtr::null();
    }

    log!(
        "Spoofing DNS request for gethostbyname({:?} \"{}\") => 127.0.0.1",
        name,
        name_str
    );

    // 2. Create a fake IP address (127.0.0.1 / localhost)
    let ip_addr = FakeIP { b1: 127, b2: 0, b3: 0, b4: 1 };
    let ip_ptr = env.mem.alloc_and_write(ip_addr);

    // 3. Create the null-terminated address list
    let addr_list = FakeAddrList { ptr: ip_ptr.to_bits(), null_term: 0 };
    let addr_list_ptr = env.mem.alloc_and_write(addr_list);

    // 4. Create the null-terminated aliases list
    let aliases = FakeAliasList { null_term: 0 };
    let aliases_ptr = env.mem.alloc_and_write(aliases);

    // 5. Populate the full hostent struct
    let hostent_data = hostent {
        h_name: name.cast_mut(),
        h_aliases: aliases_ptr.cast(),
        h_addrtype: AF_INET,
        h_length: 4, // IPv4 length
        h_addr_list: addr_list_ptr.cast(),
    };

    // Allocate the structure and pass the pointer back to the game
    env.mem.alloc_and_write(hostent_data)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
];