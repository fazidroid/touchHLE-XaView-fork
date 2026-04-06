/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SCNetworkReachability

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::cf_allocator::CFAllocatorRef;
use crate::frameworks::core_foundation::CFTypeRef;
use crate::libc::sys::socket::sockaddr;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr};
use crate::objc::{id, objc_classes, msg, Class, ClassExports, HostObject};
use crate::Environment;
use std::net::SocketAddrV4;

type SCNetworkReachabilityFlags = u32;
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
    is_name_based: bool, // Track whether this is checking an internet hostname
}
impl HostObject for SCNetworkReachabilityHostObject {}

type SCNetworkReachabilityRef = CFTypeRef;

fn SCNetworkReachabilityCreateWithName(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    _nodename: ConstPtr<i8>,
) -> SCNetworkReachabilityRef {
    let class: Class = env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem);
    
    // FIXED: Let alloc_object safely create the new object using the actual class pointer
    env.objc.alloc_object(
        class,
        Box::new(SCNetworkReachabilityHostObject { 
            address: None, 
            is_name_based: true 
        }),
        &mut env.mem,
    )
}

fn SCNetworkReachabilityCreateWithAddress(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    address: ConstPtr<sockaddr>,
) -> SCNetworkReachabilityRef {
    let addr = env.mem.read(address);
    let class: Class = env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem);
    
    // FIXED: Let alloc_object safely create the new object using the actual class pointer
    env.objc.alloc_object(
        class,
        Box::new(SCNetworkReachabilityHostObject {
            address: Some(addr.to_sockaddr_v4()),
            is_name_based: false,
        }),
        &mut env.mem,
    )
}

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
    
    let host_object = env.objc.borrow::<SCNetworkReachabilityHostObject>(target);
    
    if host_object.is_name_based {
        // Ad networks (Burstly, Tapjoy, Gameloft Trackers) use CreateWithName to check internet servers.
        // We return 0 (Unreachable) so they voluntarily disable themselves and prevent crashes!
        env.mem.write(flags, 0);
    } else {
        // Games use CreateWithAddress (0.0.0.0) to check if the Wi-Fi hardware is turned on.
        // We return Reachable so "No WIFI" alerts don't appear and Local Multiplayer works perfectly!
        let out_flags = kSCNetworkReachabilityFlagsReachable | kSCNetworkReachabilityFlagsIsDirect;
        env.mem.write(flags, out_flags);
    }
    
    true
}

fn SCNetworkReachabilitySetCallback(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    _callout: GuestFunction, 
    _context: MutVoidPtr,    
) -> bool {
    let target_class: Class = msg![env; target class];
    assert_eq!(
        target_class,
        env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem)
    );
    
    // Pretend the callback registered perfectly
    true
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _)),
    export_c_func!(SCNetworkReachabilitySetCallback(_, _, _)),
];
