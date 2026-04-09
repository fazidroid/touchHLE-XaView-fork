/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SCNetworkReachability.

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr, Ptr};
use crate::Environment;
use crate::objc::ClassExports;

pub const CLASSES: ClassExports = &[];

// Define the missing type so the compiler stops complaining
pub type SCNetworkReachabilityRef = MutVoidPtr;

fn SCNetworkReachabilityCreateWithAddress(_env: &mut Environment, _allocator: ConstPtr<u8>, _address: ConstPtr<u8>) -> SCNetworkReachabilityRef {
    Ptr::from_bits(0xDEADBEEF)
}

fn SCNetworkReachabilityCreateWithName(_env: &mut Environment, _allocator: ConstPtr<u8>, _nodename: ConstPtr<u8>) -> SCNetworkReachabilityRef {
    Ptr::from_bits(0xDEADBEEF)
}

fn SCNetworkReachabilityGetFlags(_env: &mut Environment, _target: SCNetworkReachabilityRef, flags_out: MutPtr<u32>) -> i32 {
    if !flags_out.is_null() {
        // We write `2` (kSCNetworkReachabilityFlagsReachable) instead of 0.
        // This forces the game to attempt a socket connection (which we instantly 
        // kill in netdb.rs), breaking the infinite "Gateway is down" loop!
        _env.mem.write(flags_out, 2u32); 
    }
    1 // Return true
}

fn SCNetworkReachabilityScheduleWithRunLoop(_env: &mut Environment, _target: SCNetworkReachabilityRef, _run_loop: MutVoidPtr, _run_loop_mode: MutVoidPtr) -> i32 {
    1 // Return true (Absorb safely)
}

fn SCNetworkReachabilityUnscheduleFromRunLoop(_env: &mut Environment, _target: SCNetworkReachabilityRef, _run_loop: MutVoidPtr, _run_loop_mode: MutVoidPtr) -> i32 {
    1 // Return true (Absorb safely)
}

fn SCNetworkReachabilitySetCallback(_env: &mut Environment, _target: SCNetworkReachabilityRef, _callout: MutVoidPtr, _context: MutVoidPtr) -> i32 {
    1
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _)),
    export_c_func!(SCNetworkReachabilitySetCallback(_, _, _)),
    export_c_func!(SCNetworkReachabilityScheduleWithRunLoop(_, _, _)),
    export_c_func!(SCNetworkReachabilityUnscheduleFromRunLoop(_, _, _)),
];
