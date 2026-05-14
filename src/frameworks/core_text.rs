// src/frameworks/core_text/mod.rs
//! Stubs and implementations for the Core Text framework.

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

/// Opaque type representing a Core Text font object.
/// CTFontRef is toll‑free bridged with UIFont.
pub type CTFontRef = id;

/// Core Text function to create a CTFont object from a CGFont.
pub fn CTFontCreateWithGraphicsFont(
    env: &mut Environment,
    _cg_font: CGFontRef,
    size: CGFloat,
    _transform: ConstPtr<CGAffineTransform>,
    _attributes: id,
) -> CTFontRef {
    log_dbg!(
        "CTFontCreateWithGraphicsFont called, size={}",
        size
    );

    let font_name_str = "Helvetica".to_string();
    let default_size = if size == 0.0 { 17.0 } else { size };

    let uifont_class = msg_class![env; UIFont class];
    let name_ns = ns_string::from_rust_string(env, font_name_str);
    msg![env; uifont_class fontWithName:name_ns size:default_size]
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CTFontCreateWithGraphicsFont(_, _, _, _)),
];
