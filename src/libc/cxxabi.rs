/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `cxxabi.h`
//!
//! Resources:
//! - [Itanium C++ ABI specification](https://itanium-cxx-abi.github.io/cxx-abi/abi.html#dso-dtor-runtime-api)

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::MutVoidPtr;
use crate::Environment;

fn __cxa_atexit(
    _env: &mut Environment,
    _func: GuestFunction,
    _p: MutVoidPtr,
    _d: MutVoidPtr,
) -> i32 {
    // BypassCxaAtexit
    0
}

fn __cxa_finalize(_env: &mut Environment, _d: MutVoidPtr) {
    // BypassCxaFinalize
}

fn _Unwind_SjLj_Register(_env: &mut Environment, _context: MutVoidPtr) {
    // BypassSjLjRegister
}

fn _Unwind_SjLj_Unregister(_env: &mut Environment, _context: MutVoidPtr) {
    // BypassSjLjUnregister
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(__cxa_atexit(_, _, _)),
    export_c_func!(__cxa_finalize(_)),
    export_c_func!(_Unwind_SjLj_Register(_)),
    export_c_func!(_Unwind_SjLj_Unregister(_)),
];
