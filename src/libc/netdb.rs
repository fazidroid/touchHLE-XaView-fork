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
// 🏎️ Added specifically for the Airplane Mode bypass
const EAI_NONAME: i32 = 8; 

#[allow(non_camel_case_types)]
pub type socklen_t = u32;

// Define the actual memory layout of the C hostent struct
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
    _env: &mut Environment,
    _node_name: MutPtr<u8>,
    _serv_name: MutPtr<u8>,
    _hints: ConstPtr<addrinfo>,
    _res: MutPtr<MutPtr<addrinfo>>,
) -> i32 {
    // 🏎️ GAMELOFT BYPASS: Instantly simulate "Airplane Mode" (No Internet)
    // This forces Asphalt 8 and 6 to abort the CRM retry loop and jump straight to the main menu!
    log!("🏎️ GAMELOFT BYPASS: getaddrinfo called. Simulating Airplane Mode (EAI_NONAME)!");
    EAI_NONAME
}

fn freeaddrinfo(_env: &mut Environment, _addrinfo: MutPtr<addrinfo>) {
    // Since getaddrinfo never actually allocates anything now, this safely does nothing.
}

fn gethostbyname(env: &mut Environment, name: ConstPtr<u8>) -> MutPtr<hostent> {
    let host_name = env.mem.cstr_at_utf8(name).unwrap_or("unknown").to_string();
    
    // 🏎️ GAMELOFT BYPASS: Return NULL to strictly enforce Airplane Mode for old DNS lookups!
    log!("🏎️ GAMELOFT BYPASS: Intercepted gethostbyname(\"{}\")! Enforcing Airplane Mode (NULL).", host_name);
    
    Ptr::null()
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
];
