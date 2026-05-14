// src/frameworks/core_text/mod.rs

use crate::dyld::{export_c_func, FunctionExports, HostDylib};
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::cg_font::CGFontRef;
use crate::frameworks::core_graphics::CGFloat;
use crate::frameworks::foundation::ns_string;
use crate::mem::ConstPtr;
use crate::objc::{id, msg, msg_class};
use crate::Environment;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/CoreText.framework/CoreText",
    aliases: &[],
    class_exports: &[],
    function_exports: &[FUNCTIONS],
    constant_exports: &[],
};

pub type CTFontRef = id;

#[no_mangle]
pub extern "C" fn CTFontCreateWithGraphicsFont(
    env: &mut Environment,
    _cg_font: CGFontRef,
    size: CGFloat,
    _transform: ConstPtr<CGAffineTransform>,
    _attributes: id,
) -> CTFontRef {
    log_dbg!("CTFontCreateWithGraphicsFont called, size={}", size);

    let font_name_str = "Helvetica".to_string();
    let default_size = if size == 0.0 { 17.0 } else { size };

    let uifont_class = msg_class![env; UIFont];
    let name_ns = ns_string::from_rust_string(env, font_name_str);
    msg![env; uifont_class fontWithName:name_ns size:default_size]
}

/// NEW: Stub for copying the graphics font back out of a Core Text font
#[no_mangle]
pub extern "C" fn CTFontCopyGraphicsFont(
    _env: &mut Environment,
    _font: CTFontRef,
    _attributes: ConstPtr<id>,
) -> CGFontRef {
    log_dbg!("CTFontCopyGraphicsFont stub called");
    // For now, return nil. Most games have a fallback if this returns null.
    // If the game crashes later, we may need to return a retained generic CGFont.
    crate::objc::nil
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CTFontCreateWithGraphicsFont(_, _, _, _)),
    // ADDED: Exactly 2 underscores for the 2 arguments after 'env'
    export_c_func!(CTFontCopyGraphicsFont(_, _)), 
];
