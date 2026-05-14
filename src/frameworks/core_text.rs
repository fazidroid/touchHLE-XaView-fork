// src/frameworks/core_text/mod.rs
//! Stubs and implementations for the Core Text framework.

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::cg_font::CGFontRef;
use crate::frameworks::core_graphics::{CGAffineTransform, CGFloat};
use crate::mem::ConstPtr;
use crate::objc::{id, msg_class, msg, nil};
use crate::Environment;
use crate::frameworks::foundation::ns_string;

/// Opaque type representing a Core Text font object.
/// CTFontRef is toll‑free bridged with UIFont.
pub type CTFontRef = id;

/// Core Text function to create a CTFont object from a CGFont.
#[no_mangle]
pub extern "C" fn CTFontCreateWithGraphicsFont(
    env: &mut Environment,
    cg_font: CGFontRef,
    size: CGFloat,
    _transform: ConstPtr<CGAffineTransform>,
    _attributes: id,
) -> CTFontRef {
    log_dbg!(
        "CTFontCreateWithGraphicsFont cg_font={:?}, size={}",
        cg_font,
        size
    );

    // Get the postscript name of the CGFont.
    let name = crate::frameworks::core_graphics::cg_font::CGFontCopyPostScriptName(env, cg_font);
    let font_name_str = ns_string::to_rust_string(env, name);
    log_dbg!("... postscript name: {}", font_name_str);

    let default_size: CGFloat = if size == 0.0 { 17.0 } else { size };

    // Get UIFont class object
    let uifont_class = msg_class![env; UIFont class];

    let font: id = if !font_name_str.is_empty() {
        let name_ns = ns_string::from_rust_string(env, font_name_str);
        // UIFont *font = [UIFont fontWithName:name size:default_size];
        msg![env; uifont_class fontWithName:name_ns size:default_size]
    } else {
        msg![env; uifont_class systemFontOfSize:default_size]
    };

    font
}

/// Helper function to get the name of a CGFont.
pub fn CGFontCopyPostScriptName(env: &mut Environment, _font: CGFontRef) -> id {
    ns_string::from_rust_string(env, "Helvetica".to_string())
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CTFontCreateWithGraphicsFont(_, _, _, _, _)),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/CoreText.framework/CoreText",
    aliases: &[],
    class_exports: &[],
    function_exports: FUNCTIONS,
    constant_exports: &[],
};