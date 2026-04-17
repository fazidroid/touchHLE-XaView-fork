/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SCNetworkReachability

use crate::abi::{GuestFunction, CallFromHost};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::cf_allocator::CFAllocatorRef;
use crate::frameworks::core_foundation::CFTypeRef;
use crate::libc::sys::socket::sockaddr;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr, Ptr};
use crate::objc::{objc_classes, msg, Class, ClassExports, HostObject};
use crate::Environment;
use std::net::SocketAddrV4;

type SCNetworkReachabilityFlags = u32;
const kSCNetworkReachabilityFlagsReachable: SCNetworkReachabilityFlags = 1 << 1;
const kSCNetworkReachabilityFlagsIsDirect: SCNetworkReachabilityFlags = 1 << 17;
const kSCNetworkReachabilityFlagsIsWWAN: SCNetworkReachabilityFlags = 1 << 18;

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);
    @implementation _touchHLE_SCNetworkReachability: NSObject
    @end
};

struct SCNetworkReachabilityHostObject {
    address: Option<SocketAddrV4>,
    is_name_based: bool,
    callback: Option<GuestFunction>,
    callback_context: MutVoidPtr,
}
impl HostObject for SCNetworkReachabilityHostObject {}

type SCNetworkReachabilityRef = CFTypeRef;

fn SCNetworkReachabilityCreateWithName(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    _nodename: ConstPtr<i8>,
) -> SCNetworkReachabilityRef {
    let class: Class = env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem);
    env.objc.alloc_object(
        class,
        Box::new(SCNetworkReachabilityHostObject {
            address: None,
            is_name_based: true,
            callback: None,
            callback_context: Ptr::null(),
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
    env.objc.alloc_object(
        class,
        Box::new(SCNetworkReachabilityHostObject {
            address: Some(addr.to_sockaddr_v4()),
            is_name_based: false,
            callback: None,
            callback_context: Ptr::null(),
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

    // Always report reachable (WiFi + WWAN) to satisfy Asphalt 8.
    let out_flags = kSCNetworkReachabilityFlagsReachable | kSCNetworkReachabilityFlagsIsDirect | kSCNetworkReachabilityFlagsIsWWAN;
    env.mem.write(flags, out_flags);
    true
}

fn SCNetworkReachabilitySetCallback(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    callout: GuestFunction,
    context: MutVoidPtr,
) -> bool {
    let target_class: Class = msg![env; target class];
    assert_eq!(
        target_class,
        env.objc.get_known_class("_touchHLE_SCNetworkReachability", &mut env.mem)
    );

    let mut host_object = env.objc.borrow_mut::<SCNetworkReachabilityHostObject>(target);
    host_object.callback = Some(callout);
    host_object.callback_context = context;
    log_dbg!("SCNetworkReachabilitySetCallback: stored callback");
    true
}

fn SCNetworkReachabilityScheduleWithRunLoop(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    _run_loop: MutVoidPtr,
    _run_loop_mode: MutVoidPtr,
) -> bool {
    let host_object = env.objc.borrow::<SCNetworkReachabilityHostObject>(target);
    if let Some(callback) = host_object.callback {
        let flags = kSCNetworkReachabilityFlagsReachable | kSCNetworkReachabilityFlagsIsDirect | kSCNetworkReachabilityFlagsIsWWAN;
        let context = host_object.callback_context;
               log_dbg!("SCNetworkReachabilityScheduleWithRunLoop: firing callback with flags {:#x}", flags);
        let _: u32 = callback.call_from_host(env, (target, flags, context));
    }
    true
}

fn SCNetworkReachabilityUnscheduleFromRunLoop(
    _env: &mut Environment,
    _target: SCNetworkReachabilityRef,
    _run_loop: MutVoidPtr,
    _run_loop_mode: MutVoidPtr,
) -> bool {
    true
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _)),
    export_c_func!(SCNetworkReachabilitySetCallback(_, _, _)),
    export_c_func!(SCNetworkReachabilityScheduleWithRunLoop(_, _, _)),
    export_c_func!(SCNetworkReachabilityUnscheduleFromRunLoop(_, _, _)),
];