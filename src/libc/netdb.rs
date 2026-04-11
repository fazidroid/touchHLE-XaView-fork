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

const EAI_FAIL: i32 = 4;

#[allow(non_camel_case_types)]
pub type socklen_t = u32;

// 🏎️ GAMELOFT BYPASS: Define the actual memory layout of the C hostent struct
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

// Helper structs to cleanly write C-style arrays into guest memory
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct AddrList {
    ip: MutPtr<u8>,
    null_ptr: MutPtr<u8>,
}
unsafe impl SafeRead for AddrList {}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct AliasesList {
    null_ptr: MutPtr<u8>,
}
unsafe impl SafeRead for AliasesList {}

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
    let host_name = env.mem.cstr_at_utf8(name).unwrap_or("unknown");
    log!("🏎️ GAMELOFT BYPASS: Intercepted gethostbyname(\"{}\")! Redirecting to 127.0.0.1", host_name);

    // 1. Allocate a copy of the host name using raw bytes to satisfy the compiler
    let h_name_ptr = env.mem.alloc_and_write_cstr(host_name.as_bytes()).cast::<u8>();

    // 2. Allocate the 127.0.0.1 IP. We encode it as a Little-Endian u32 
    // so it perfectly matches the memory layout of [127, 0, 0, 1] without needing array traits!
    let ip_data = u32::from_le_bytes([127, 0, 0, 1]);
    let ip_ptr = env.mem.alloc_and_write(ip_data).cast::<u8>();

    // 3. Construct and allocate the h_addr_list
    let addr_list_data = AddrList {
        ip: ip_ptr,
        null_ptr: Ptr::null(),
    };
    let addr_list_ptr = env.mem.alloc_and_write(addr_list_data).cast::<MutPtr<u8>>();

    // 4. Construct and allocate the h_aliases array
    let aliases_data = AliasesList {
        null_ptr: Ptr::null(),
    };
    let aliases_ptr = env.mem.alloc_and_write(aliases_data).cast::<MutPtr<u8>>();

    // 5. Construct the final hostent struct
    let hostent_data = hostent {
        h_name: h_name_ptr,
        h_aliases: aliases_ptr,
        h_addrtype: AF_INET, // AF_INET is typically 2
        h_length: 4,         // IPv4 length is 4 bytes
        h_addr_list: addr_list_ptr,
    };

    // 6. Write the struct to guest memory and return the pointer
    env.mem.alloc_and_write(hostent_data)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
];
