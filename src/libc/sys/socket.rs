/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `sys/socket.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::errno::{set_errno, EPROTONOSUPPORT};
use crate::Environment;

pub const AF_INET: i32 = 2;

fn socket(env: &mut Environment, domain: i32, type_: i32, protocol: i32) -> i32 {
    log!(
        "Warning: socket({}, {}, {}) is unimplemented, returning -1",
        domain,
        type_,
        protocol
    );
    set_errno(env, EPROTONOSUPPORT);
    -1
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(socket(_, _, _))];
