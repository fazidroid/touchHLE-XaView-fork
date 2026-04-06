/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SCNetworkReachability

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use crate::frameworks::core_foundation::CFTypeRef;
use crate::libc::sys::socket::sockaddr;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr, Ptr};
use crate::objc::{objc_classes, msg, Class, ClassExports, HostObject};
use crate::Environment;
use std::net::SocketAddrV4;

type SCNetworkReachabilityFlags = u32;
// Flag indicating the network is fully reachable
const kSCNetworkReachabilityFlagsReachable: SCNetworkReachabilityFlags = 1 << 1;
#[allow(dead_code)]
const kSCNetworkReachabilityFlagsIsDirect: SCNetworkReachabilityFlags = 1 << 17;

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);
    @implementation _touchHLE_SCNetworkReachability: NSObject
    @end
};

#[allow(dead_code)]
struct SCNetworkReachabilityHostObject {
    address: Option<SocketAddrV4>,
}
impl HostObject for SCNetworkReachabilityHostObject {}

type SCNetworkReachabilityRef = CFTypeRef;

fn SCNetworkReachabilityCreateWithName(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    _nodename: ConstPtr<i8>,
) -> SCNetworkReachabilityRef {
    let class: Class = env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem);
    let res: id = msg![env; class alloc];
    env.objc.alloc_object(
        res,
        Box::new(SCNetworkReachabilityHostObject { address: None }),
        &mut env.mem,
    );
    res
}

fn SCNetworkReachabilityCreateWithAddress(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    address: ConstPtr<sockaddr>,
) -> SCNetworkReachabilityRef {
    let addr = env.mem.read(address);
    let class: Class = env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem);
    let res: id = msg![env; class alloc];
    env.objc.alloc_object(
        res,
        Box::new(SCNetworkReachabilityHostObject {
            address: Some(addr.to_sockaddr_v4()),
        }),
        &mut env.mem,
    );
    res
}

// ===== ALWAYS ONLINE BYPASS =====
fn SCNetworkReachabilityGetFlags(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    flags: MutPtr<SCNetworkReachabilityFlags>,
) -> bool {
    let target_class: Class = msg![env; target class];
    assert_eq!(
        target_class,
        env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem)
    );
    
    // Always tell the game we have an active, reachable connection!
    // This stops games like GT Racing from freezing due to "No WIFI" alerts.
    let out_flags = kSCNetworkReachabilityFlagsReachable;
    env.mem.write(flags, out_flags);
    
    true
}
// ================================

fn SCNetworkReachabilitySetCallback(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    _callout: GuestFunction, // Fixed unused warning
    _context: MutVoidPtr,    // Fixed unused warning
) -> bool {
    let target_class: Class = msg![env; target class];
    assert_eq!(
        target_class,
        env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem)
    );
    
    // Just return true to pretend we successfully registered the callback
    true
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _, _)),
    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _, _)),
    export_c_func!(SCNetworkReachabilitySetCallback(_, _, _, _)),
];
