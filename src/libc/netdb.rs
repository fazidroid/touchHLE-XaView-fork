/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `netdb.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::sys::socket::{sockaddr, AF_INET, SOCK_DGRAM, SOCK_STREAM};
use crate::mem::{guest_size_of, ConstPtr, MutPtr, Ptr, SafeRead, SafeWrite};
use crate::Environment;

const AI_PASSIVE: i32 = 0x1;

pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;

const EAI_FAIL: i32 = 4;

#[allow(non_camel_case_types)]
pub type socklen_t = u32;

#[derive(Default)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
pub struct hostent {
    pub h_name: MutPtr<u8>,
    pub h_aliases: MutPtr<MutPtr<u8>>,
    pub h_addrtype: i32,
    pub h_length: i32,
    pub h_addr_list: MutPtr<MutPtr<u8>>,
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
    node_name: MutPtr<u8>,
    serv_name: MutPtr<u8>,
    hints: ConstPtr<addrinfo>,
    res: MutPtr<MutPtr<addrinfo>>,
) -> i32 {
    if !env.options.network_access {
        log_dbg!(
            "Network access disabled, getaddrinfo({:?}, {:?}, ...) -> EAI_FAIL",
            node_name,
            serv_name
        );
        return EAI_FAIL;
    }

    // Parse hints or use defaults
    let (ai_flags, ai_family, ai_socktype, ai_protocol, ai_addrlen) = if hints.is_null() {
        (0, AF_INET, SOCK_STREAM, 0, 0)
    } else {
        let hint = env.mem.read(hints);
        (
            hint.ai_flags,
            hint.ai_family,
            hint.ai_socktype,
            hint.ai_protocol,
            hint.ai_addrlen,
        )
    };

    // Only support AF_INET for now
    if ai_family != AF_INET && ai_family != 0 {
        log_dbg!("getaddrinfo: unsupported ai_family {}", ai_family);
        return EAI_FAIL;
    }

    // Parse port number from serv_name (e.g., "80")
    let port_str = env.mem.cstr_at_utf8(serv_name).unwrap_or_default();
    let port: u16 = match port_str.parse() {
        Ok(p) => p,
        Err(_) => {
            log_dbg!("getaddrinfo: invalid port '{}'", port_str);
            return EAI_FAIL;
        }
    };

    // Determine the IP address to return
    let ip_addr = if node_name.is_null() {
        // Passive mode: bind to any address (0.0.0.0)
        log_dbg!("getaddrinfo: passive mode (node_name null), binding to 0.0.0.0:{}", port);
        [0, 0, 0, 0]
    } else {
        // Active mode: return localhost for any hostname (stub for Asphalt 8)
        let hostname = env.mem.cstr_at_utf8(node_name).unwrap_or_default();
        log_dbg!("getaddrinfo: active mode for '{}', returning 127.0.0.1:{}", hostname, port);
        [127, 0, 0, 1]
    };

    // Build the sockaddr_in
    let addr = sockaddr::from_ipv4_parts(ip_addr, port);
    let addr_ptr = env.mem.alloc_and_write(addr);

    // Build the addrinfo structure
    let ai = addrinfo {
        ai_flags,
        ai_family: AF_INET,
        ai_socktype,
        ai_protocol,
        ai_addrlen: guest_size_of::<sockaddr>(),
        ai_canonname: if node_name.is_null() {
            MutPtr::null()
        } else {
            // Optional: copy the hostname as canonical name
            node_name
        },
        ai_addr: addr_ptr,
        ai_next: MutPtr::null(),
    };
    let ai_ptr = env.mem.alloc_and_write(ai);
    env.mem.write(res, ai_ptr);

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
    let name_str = env.mem.cstr_at_utf8(name).unwrap_or_default().to_string();
    
    // ==========================================================
    // 🏎️ GT RACING EXCLUSIVE BYPASS: Offline Telemetry Loop
    // ==========================================================
    let main_bundle: crate::objc::id = crate::objc::msg_class![env; NSBundle mainBundle];
    let mut is_gt_racing = false;
    
    if main_bundle != crate::objc::nil {
        let bundle_id: crate::objc::id = crate::objc::msg![env; main_bundle bundleIdentifier];
        if bundle_id != crate::objc::nil {
            let bundle_str = crate::frameworks::foundation::ns_string::to_rust_string(env, bundle_id);
            // Strictly check for the exact GT Racing bundle IDs
            is_gt_racing = bundle_str == "com.gameloft.GTRacingFreemiumHD" ||
                           bundle_str == "com.gameloft.GTRacingFreemium" ||
                           bundle_str == "com.gameloft.GTRacingFreemiumUK";
        }
    }

    if is_gt_racing {
        println!("🎮 GT RACING EXCLUSIVE: Spoofing gethostbyname for [{}]", name_str);
        
        // 1. Allocate the IP data (127.0.0.1) safely in its own 4-byte block
        let ip_data = env.mem.alloc(4).cast::<u8>();
        env.mem.write(ip_data + 0, 127);
        env.mem.write(ip_data + 1, 0);
        env.mem.write(ip_data + 2, 0);
        env.mem.write(ip_data + 3, 1);

        // 2. Allocate the h_addr_list (Array of pointers, terminated by NULL)
        let addr_list = env.mem.alloc(2).cast::<crate::mem::MutPtr<u8>>();
        env.mem.write(addr_list + 0, ip_data);
        env.mem.write(addr_list + 1, crate::mem::MutPtr::null());

        // 3. Allocate the h_aliases (Array of pointers, terminated by NULL)
        let aliases = env.mem.alloc(1).cast::<crate::mem::MutPtr<u8>>();
        env.mem.write(aliases + 0, crate::mem::MutPtr::null());

        // 4. Construct the hostent struct and write it safely!
        let h = hostent {
            h_name: name.cast_mut(),
            h_aliases: aliases,
            h_addrtype: 2, // AF_INET
            h_length: 4,
            h_addr_list: addr_list,
        };
        
        return env.mem.alloc_and_write(h);
    }

    // Standard touchHLE behavior for all other games
    log!("TODO: gethostbyname({:?} {:?}) => NULL", name, name_str);
    crate::mem::MutPtr::null()
}

fn inet_ntoa(env: &mut Environment, in_addr: u32) -> MutPtr<u8> {
    // Decode the 32-bit IPv4 address into 4 bytes
    let b1 = (in_addr & 0xFF) as u8;
    let b2 = ((in_addr >> 8) & 0xFF) as u8;
    let b3 = ((in_addr >> 16) & 0xFF) as u8;
    let b4 = ((in_addr >> 24) & 0xFF) as u8;
    
    let ip_str = format!("{}.{}.{}.{}", b1, b2, b3, b4);
    println!("🎮 GT RACING EXCLUSIVE: inet_ntoa converting IP to string: {}", ip_str);
    
        // Allocate the string in guest memory and return the pointer
    env.mem.alloc_and_write_cstr(ip_str.as_bytes())
}


pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getaddrinfo(_, _, _, _)),
    export_c_func!(freeaddrinfo(_)),
    export_c_func!(gethostbyname(_)),
    export_c_func!(inet_ntoa(_)),
];
