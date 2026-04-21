/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGFont.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::frameworks::core_foundation::{CFRelease, CFRetain, CFTypeRef};
use crate::mem::ConstVoidPtr;
use crate::objc::{objc_classes, ClassExports, HostObject};
use crate::Environment;

pub type CGFontRef = CFTypeRef;

struct CGFontHostObject;
impl HostObject for CGFontHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation _touchHLE_CGFont: NSObject

- (())dealloc {
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};

pub fn CGFontRelease(env: &mut Environment, font: CGFontRef) {
    if !font.is_null() {
        CFRelease(env, font);
    }
}

pub fn CGFontRetain(env: &mut Environment, font: CGFontRef) -> CGFontRef {
    if !font.is_null() {
        CFRetain(env, font)
    } else {
        font
    }
}

// Private function stub - note the leading underscore in the Rust name
fn _CGFontCreateWithDataProvider(
    env: &mut Environment,
    _provider: CFTypeRef, // CGDataProviderRef
) -> CGFontRef {
    log!("_CGFontCreateWithDataProvider stub called");
    // Create a dummy font object to avoid null crashes
    let class = env
        .objc
        .get_known_class("_touchHLE_CGFont", &mut env.mem);
    env.objc.alloc_object(class, Box::new(CGFontHostObject), &mut env.mem)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGFontRelease(_)),
    export_c_func!(CGFontRetain(_)),
    // Manual export with single underscore prefix
    ("_CGFontCreateWithDataProvider", &(_CGFontCreateWithDataProvider as fn(&mut Environment, CFTypeRef) -> CGFontRef)),
];
