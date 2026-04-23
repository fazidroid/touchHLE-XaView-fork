/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFSocket`

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::CFTypeRef;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{MutVoidPtr, Ptr};
use crate::Environment;

fn CFSocketCreate(
    _env: &mut Environment,
    allocator: CFAllocatorRef,
    protocol_family: i32,
    type_: i32,
    protocol: i32,
    flags: u32,
    callout: MutVoidPtr,
    context: MutVoidPtr,
) -> CFTypeRef {
    assert_eq!(allocator, kCFAllocatorDefault); // unimplemented
    log!(
        "TODO: CFSocketCreate({}, {}, {}, {}, {:?}, {:?}) -> NULL",
        protocol_family,
        type_,
        protocol,
        flags,
        callout,
        context
    );
    Ptr::null()
}

// ==========================================================
// 🏎️ EA/FIREMINT BYPASS: Stub CFHTTPMessageCreateRequest
// ==========================================================
fn CFHTTPMessageCreateRequest(
    _env: &mut crate::Environment,
    _alloc: crate::objc::id,
    _request_method: crate::objc::id,
    _url: crate::objc::id,
    _http_version: crate::objc::id,
) -> crate::objc::id {
    println!("🎮 LOG: Caught CFHTTPMessageCreateRequest. Forcing offline mode!");
    crate::objc::nil
}

fn CFHTTPMessageSetHeaderFieldValue(
    _env: &mut Environment,
    _message: CFTypeRef,
    _header_field: CFTypeRef,
    _value: CFTypeRef,
) {
    log!("🎮 LOG: Caught CFHTTPMessageSetHeaderFieldValue. Absorbing safely!");
}

// ==========================================================
// 🏎️ EA/FIREMINT BYPASS: Stub CFHTTPMessageSetBody
// ==========================================================
fn CFHTTPMessageSetBody(
    _env: &mut Environment,
    _message: CFTypeRef,
    _body_data: CFTypeRef,
) {
    log!("🎮 LOG: Caught CFHTTPMessageSetBody. Absorbing safely!");
}

fn CFReadStreamCreateForHTTPRequest(
    _env: &mut Environment,
    _alloc: CFTypeRef,
    _request: CFTypeRef,
) -> CFTypeRef {
    log!("🎮 LOG: Caught CFReadStreamCreateForHTTPRequest. Returning null stream to force offline mode!");
    Ptr::null()
}

// 🏎️ PROACTIVE STUBS: Just in case the game checks its work!
fn CFHTTPMessageCopyHeaderFieldValue(
    _env: &mut Environment,
    _message: CFTypeRef,
    _header_field: CFTypeRef,
) -> CFTypeRef {
    Ptr::null()
}

fn CFHTTPMessageCopyAllHeaderFields(
    _env: &mut Environment,
    _message: CFTypeRef,
) -> CFTypeRef {
    Ptr::null()
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFSocketCreate(_, _, _, _, _, _, _)),
    export_c_func!(CFHTTPMessageCreateRequest(_, _, _, _)),
    export_c_func!(CFHTTPMessageSetHeaderFieldValue(_, _, _)),
    export_c_func!(CFHTTPMessageSetBody(_, _)),
    export_c_func!(CFReadStreamCreateForHTTPRequest(_, _)),
    export_c_func!(CFHTTPMessageCopyHeaderFieldValue(_, _)),
    export_c_func!(CFHTTPMessageCopyAllHeaderFields(_)),
];
