/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Mach thread actions for ARM arch.

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::mach::port::{mach_port_t, MACH_PORT_DEAD, MACH_PORT_NULL};
use crate::libc::mach::thread_info::{kern_return_t, KERN_SUCCESS};
use crate::{Environment, ThreadId};

type thread_inspect_t = mach_port_t;

fn thread_suspend(env: &mut Environment, target_thread: thread_inspect_t) -> kern_return_t {
    assert!(target_thread != MACH_PORT_NULL && target_thread != MACH_PORT_DEAD);
    // Expected `thread send right` is thread_id + 1. See `mach_thread_self()`
    env.suspend_thread((target_thread - 1) as ThreadId);
    KERN_SUCCESS
}

fn thread_resume(env: &mut Environment, target_thread: thread_inspect_t) -> kern_return_t {
    assert!(target_thread != MACH_PORT_NULL && target_thread != MACH_PORT_DEAD);
    // Expected `thread send right` is thread_id + 1. See `mach_thread_self()`
    env.resume_thread((target_thread - 1) as ThreadId);
    KERN_SUCCESS
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(thread_suspend(_)),
    export_c_func!(thread_resume(_)),
];
