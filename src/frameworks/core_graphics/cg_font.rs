/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGFont.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::frameworks::core_foundation::{CFRelease, CFRetain, CFTypeRef};
use crate::frameworks::foundation::ns_string;
use crate::frameworks::core_graphics::cg_geometry::{CGRect, CGPointZero, CGSizeZero};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr};
use crate::objc::{objc_classes, ClassExports, HostObject};
use crate::Environment;

pub type CGFontRef = CFTypeRef;
pub type CFStringRef = CFTypeRef; // Added for clarity

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

fn _CGFontCreateWithDataProvider(
    env: &mut Environment,
    _provider: CFTypeRef,
) -> CGFontRef {
    log!("_CGFontCreateWithDataProvider stub called");
    let class = env
        .objc
        .get_known_class("_touchHLE_CGFont", &mut env.mem);
    env.objc.alloc_object(class, Box::new(CGFontHostObject), &mut env.mem)
}

fn _CGFontGetUnitsPerEm(_env: &mut Environment, _font: CGFontRef) -> i32 {
    log!("_CGFontGetUnitsPerEm stub called -> returning 1000");
    1000
}

fn _CGFontGetAscent(_env: &mut Environment, _font: CGFontRef) -> i32 {
    // 🏎️ NFS FIX: Silenced the log spam to prevent I/O bottleneck freezes!
    800
}

fn _CGFontGetDescent(_env: &mut Environment, _font: CGFontRef) -> i32 {
    // 🏎️ NFS FIX: Silenced the log spam!
    -200
}

fn _CGFontGetLeading(_env: &mut Environment, _font: CGFontRef) -> i32 {
    // 🏎️ NFS FIX: Silenced the log spam!
    100
}

fn _CGFontGetGlyphAdvances(
    _env: &mut Environment,
    _font: CGFontRef,
    _glyphs: ConstPtr<u16>,
    _count: GuestUSize,
    _advances: MutPtr<i32>,
) -> i32 {
    log!("_CGFontGetGlyphAdvances stub called");
    0
}

fn _CGFontCopyFullName(env: &mut Environment, _font: CGFontRef) -> CFStringRef {
    log!("_CGFontCopyFullName stub called -> returning 'Helvetica'");
    // Use NSString (toll-free bridged to CFString)
    ns_string::from_rust_string(env, "Helvetica".to_string())
}

fn _CGFontGetGlyphBBoxes(
    _env: &mut Environment,
    _font: CGFontRef,
    _glyphs: ConstPtr<u16>,
    _count: GuestUSize,
    _bboxes: MutPtr<CGRect>,
) -> CGRect {
    log!("_CGFontGetGlyphBBoxes stub called");
    CGRect {
        origin: CGPointZero,
        size: CGSizeZero,
    }
}

pub fn CGFontCopyPostScriptName(env: &mut Environment, _font: CGFontRef) -> CFStringRef {
    log!("CGFontCopyPostScriptName stub called -> returning 'Helvetica'");
    // Return a fallback Helvetica string so EA/Firemint games don't panic!
    ns_string::from_rust_string(env, "Helvetica".to_string())
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGFontRelease(_)),
    export_c_func!(CGFontRetain(_)),
    export_c_func!(CGFontCopyPostScriptName(_)),
    ("_CGFontCreateWithDataProvider", &(_CGFontCreateWithDataProvider as fn(&mut Environment, CFTypeRef) -> CGFontRef)),
    ("_CGFontGetUnitsPerEm", &(_CGFontGetUnitsPerEm as fn(&mut Environment, CGFontRef) -> i32)),
    ("_CGFontGetAscent", &(_CGFontGetAscent as fn(&mut Environment, CGFontRef) -> i32)),
    ("_CGFontGetDescent", &(_CGFontGetDescent as fn(&mut Environment, CGFontRef) -> i32)),
    ("_CGFontGetLeading", &(_CGFontGetLeading as fn(&mut Environment, CGFontRef) -> i32)),
    ("_CGFontGetGlyphAdvances", &(_CGFontGetGlyphAdvances as fn(&mut Environment, CGFontRef, ConstPtr<u16>, GuestUSize, MutPtr<i32>) -> i32)),
    ("_CGFontCopyFullName", &(_CGFontCopyFullName as fn(&mut Environment, CGFontRef) -> CFStringRef)),
    ("_CGFontGetGlyphBBoxes", &(_CGFontGetGlyphBBoxes as fn(&mut Environment, CGFontRef, ConstPtr<u16>, GuestUSize, MutPtr<CGRect>) -> CGRect)),
];
