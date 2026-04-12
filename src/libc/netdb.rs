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
const EAI_NONAME: i32 = 8; 

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
    _env: &mut Environment,
    _node_name: MutPtr<u8>,
    _serv_name: MutPtr<u8>,
    _hints: ConstPtr<addrinfo>,
    _res: MutPtr<MutPtr<addrinfo>>,
) -> i32 {
    log!("🏎️ ASPHALT 8 BYPASS: Simulating Airplane Mode (EAI_NONAME) to break CRM loop!");
    EAI_NONAME
}

fn freeaddrinfo(_env: &mut Environment, _addrinfo: MutPtr<addrinfo>) {}

fn gethostbyname(env: &mut Environment, name: ConstPtr<u8>) -> MutPtr<hostent> {
    let host_name = env.mem.cstr_at_utf8(name).unwrap_or("unknown").to_string();
    log!("🏎️ ASPHALT 8 BYPASS: gethostbyname(\"{}\") -> NULL", host_name);
    Ptr::null()
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
];
