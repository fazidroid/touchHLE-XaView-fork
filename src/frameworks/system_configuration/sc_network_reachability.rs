/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SCNetworkReachability.

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::MutPtr;
use crate::objc::ClassExports;
use crate::Environment;

pub const CLASSES: ClassExports = &[];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _)),
    export_c_func!(SCNetworkReachabilitySetCallback(_, _, _)),
    export_c_func!(SCNetworkReachabilityScheduleWithRunLoop(_, _, _)),
    export_c_func!(SCNetworkReachabilityUnscheduleFromRunLoop(_, _, _)),
];

fn SCNetworkReachabilityCreateWithAddress(_env: &mut Environment, _allocator: u32, _address: u32) -> u32 {
    1 // Return a dummy handle
}

fn SCNetworkReachabilityCreateWithName(_env: &mut Environment, _allocator: u32, _nodename: u32) -> u32 {
    1 // Return a dummy handle
}

fn SCNetworkReachabilityGetFlags(env: &mut Environment, _target: u32, flags_out: MutPtr<u32>) -> u32 {
    if !flags_out.is_null() {
        // FIXED: Gameloft Asphalt 6 Loop Bypass!
        // We write `2` (kSCNetworkReachabilityFlagsReachable) instead of 0.
        // This forces the game to attempt a socket connection (which we instantly 
        // kill in netdb.rs), breaking the infinite "Gateway is down" loop!
        env.mem.write(flags_out, 2u32);
    }
    1 // Return 1 (true) to indicate we successfully retrieved the flags
}

fn SCNetworkReachabilitySetCallback(_env: &mut Environment, _target: u32, _callout: u32, _context: u32) -> u32 {
    1
}

fn SCNetworkReachabilityScheduleWithRunLoop(_env: &mut Environment, _target: u32, _runloop: u32, _runloop_mode: u32) -> u32 {
    1
}

fn SCNetworkReachabilityUnscheduleFromRunLoop(_env: &mut Environment, _target: u32, _runloop: u32, _runloop_mode: u32) -> u32 {
    1
}