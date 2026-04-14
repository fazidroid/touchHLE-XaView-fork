/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! MobileCoreServices.framework stubs.
//! Provides UTType functions to satisfy EA engine checks.

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::foundation::ns_string;
use crate::mem::ConstVoidPtr;
use crate::objc::{id, msg_class};
use crate::Environment;

/// UTTypeCreatePreferredIdentifierForTag
fn UTTypeCreatePreferredIdentifierForTag(
    env: &mut Environment,
    _inTagClass: id,
    inTag: id,
    _inConformingToUTI: id,
) -> id {
    log_dbg!("UTTypeCreatePreferredIdentifierForTag called");
    if !inTag.is_null() {
        inTag
    } else {
        ns_string::get_static_str(env, "public.data")
    }
}

/// UTTypeCopyDeclaration
fn UTTypeCopyDeclaration(env: &mut Environment, _inUTI: id) -> id {
    log_dbg!("UTTypeCopyDeclaration called");
    msg_class![env; NSMutableDictionary dictionary]
}

/// UTTypeConformsTo
fn UTTypeConformsTo(_env: &mut Environment, _inUTI: id, _inConformsToUTI: id) -> bool {
    log_dbg!("UTTypeConformsTo called");
    true
}

/// UTTypeCopyDescription
fn UTTypeCopyDescription(env: &mut Environment, _inUTI: id) -> id {
    log_dbg!("UTTypeCopyDescription called");
    ns_string::get_static_str(env, "Uniform Type Identifier")
}

/// UTTypeEqual
fn UTTypeEqual(_env: &mut Environment, inUTI1: id, inUTI2: id) -> bool {
    if inUTI1.is_null() || inUTI2.is_null() {
        return false;
    }
    inUTI1 == inUTI2
}

/// UTTypeCreateAllIdentifiersForTag
fn UTTypeCreateAllIdentifiersForTag(
    env: &mut Environment,
    inTagClass: id,
    inTag: id,
    inConformingToUTI: id,
) -> id {
    log_dbg!("UTTypeCreateAllIdentifiersForTag called");
    let array: id = msg_class![env; NSMutableArray array];
    let uti = UTTypeCreatePreferredIdentifierForTag(env, inTagClass, inTag, inConformingToUTI);
    let _: () = msg![env; array addObject:uti];
    array
}

/// UTTypeCopyPreferredTagWithClass
fn UTTypeCopyPreferredTagWithClass(
    env: &mut Environment,
    _inUTI: id,
    _inTagClass: id,
) -> id {
    log_dbg!("UTTypeCopyPreferredTagWithClass called");
    ns_string::get_static_str(env, "dat")
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(UTTypeCreatePreferredIdentifierForTag(_, _, _)),
    export_c_func!(UTTypeCopyDeclaration(_)),
    export_c_func!(UTTypeConformsTo(_, _)),
    export_c_func!(UTTypeCopyDescription(_)),
    export_c_func!(UTTypeEqual(_, _)),
    export_c_func!(UTTypeCreateAllIdentifiersForTag(_, _, _)),
    export_c_func!(UTTypeCopyPreferredTagWithClass(_, _)),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/MobileCoreServices.framework/MobileCoreServices",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[FUNCTIONS],
};