/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `arpa/inet.h` (Internet address manipulation routines)

use crate::libc::netdb::socklen_t;
use crate::libc::sys::socket::AF_INET;

// AF_INET6 = 30 on Darwin/iOS (matches the value the game passes)
const AF_INET6: i32 = 30;
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, SafeRead};
use crate::{export_c_func, Environment};
use crate::abi::GuestArg;

use crate::dyld::FunctionExports;
use std::net::{Ipv4Addr, Ipv6Addr};

#[allow(non_camel_case_types)]
type in_addr_t = u32;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
struct in_addr {
    s_addr: in_addr_t,
}
unsafe impl SafeRead for in_addr {}

impl GuestArg for in_addr {
    const REG_COUNT: usize = 1;

    fn from_regs(regs: &[u32]) -> Self {
        in_addr { s_addr: regs[0] }
    }

    fn to_regs(self, regs: &mut [u32]) {
        regs[0] = self.s_addr;
    }
}

fn inet_addr(env: &mut Environment, str: ConstPtr<u8>) -> in_addr_t {
    let inet_addr_str = env.mem.cstr_at_utf8(str).unwrap_or("");
    match inet_addr_str.parse::<Ipv4Addr>() {
        Ok(address) => {
            let res = u32::from_le_bytes(address.octets());
            log_dbg!("inet_addr({:?}) => {}", inet_addr_str, res);
            res
        }
        Err(_) => {
            log_dbg!("inet_addr({:?}) => INADDR_NONE", inet_addr_str);
            0xffffffff // INADDR_NONE
        }
    }
}

fn inet_ntop(
    env: &mut Environment,
    af: i32,
    src: ConstVoidPtr,
    dst: MutPtr<u8>,
    size: socklen_t,
) -> ConstPtr<u8> {
    if af == AF_INET {
        let addr_ptr: ConstPtr<in_addr> = src.cast();
        let addr = env.mem.read(addr_ptr);
        let ipv4_addr = Ipv4Addr::from_bits(u32::from_be(addr.s_addr));
        log_dbg!("inet_ntop AF_INET: addr = {:?}", ipv4_addr);
        let binding = ipv4_addr.to_string();
        let addr_bytes = binding.as_bytes();
        let len: GuestUSize = addr_bytes.len().try_into().unwrap();
        if len >= size {
            log!("inet_ntop: buffer too small ({} < {})", size, len);
            return crate::mem::Ptr::null();
        }
        env.mem.bytes_at_mut(dst, len).copy_from_slice(addr_bytes);
        env.mem.write(dst + len, b'\0');
        dst.cast_const()
    } else if af == AF_INET6 {
        // Read 16-byte IPv6 address from src
        let bytes_ptr = src.cast::<u8>();
        let mut octets = [0u8; 16];
        for i in 0..16u32 {
            octets[i as usize] = env.mem.read(bytes_ptr + i);
        }
        let ipv6_addr = Ipv6Addr::from(octets);
        log_dbg!("inet_ntop AF_INET6: addr = {:?}", ipv6_addr);
        let binding = ipv6_addr.to_string();
        let addr_bytes = binding.as_bytes();
        let len: GuestUSize = addr_bytes.len().try_into().unwrap();
        if len >= size {
            log!("inet_ntop AF_INET6: buffer too small ({} < {})", size, len);
            return crate::mem::Ptr::null();
        }
        env.mem.bytes_at_mut(dst, len).copy_from_slice(addr_bytes);
        env.mem.write(dst + len, b'\0');
        dst.cast_const()
    } else {
        log!("inet_ntop: unsupported address family {}, returning null", af);
        crate::mem::Ptr::null()
    }
}

fn inet_pton(env: &mut Environment, af: i32, src: ConstPtr<u8>, dst: MutVoidPtr) -> i32 {
    let str = env.mem.cstr_at_utf8(src.cast()).unwrap_or("");
    log_dbg!("inet_pton af={} '{}'", af, str);

    if af == AF_INET {
        match str.parse::<Ipv4Addr>() {
            Ok(address) => {
                let addr = in_addr {
                    s_addr: u32::from_le_bytes(address.octets()),
                };
                let addr_ptr: MutPtr<in_addr> = dst.cast();
                env.mem.write(addr_ptr, addr);
                1 // success
            }
            Err(_) => {
                log_dbg!("inet_pton AF_INET: invalid address '{}'", str);
                0
            }
        }
    } else if af == AF_INET6 {
        match str.parse::<Ipv6Addr>() {
            Ok(address) => {
                let octets = address.octets();
                let bytes_ptr = dst.cast::<u8>();
                for (i, &b) in octets.iter().enumerate() {
                    env.mem.write(bytes_ptr + i as u32, b);
                }
                1 // success
            }
            Err(_) => {
                log_dbg!("inet_pton AF_INET6: invalid address '{}'", str);
                0
            }
        }
    } else {
        log!("inet_pton: unsupported address family {}, returning -1", af);
        -1
    }
}

fn inet_ntoa(env: &mut Environment, addr: in_addr) -> MutPtr<u8> {
    let ipv4_addr = Ipv4Addr::from_bits(u32::from_be(addr.s_addr));
    let ip_str = ipv4_addr.to_string();
    let len = ip_str.len();
    let buf = env.mem.alloc(len as u32 + 1).cast::<u8>();
    let slice = env.mem.bytes_at_mut(buf, len as u32 + 1);
    slice[..len].copy_from_slice(ip_str.as_bytes());
    slice[len] = b'\0';
    buf
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(inet_addr(_)),
    export_c_func!(inet_ntop(_, _, _, _)),
    export_c_func!(inet_pton(_, _, _)),
    ("_inet_ntoa", &(inet_ntoa as fn(&mut Environment, in_addr) -> MutPtr<u8>)),
];
