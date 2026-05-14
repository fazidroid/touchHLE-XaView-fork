// src/frameworks/core_text.rs

use crate::dyld::{export_c_func, FunctionExports, HostDylib};
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::cg_font::CGFontRef;
use crate::frameworks::core_graphics::CGFloat;
use crate::frameworks::foundation::ns_string;
use crate::mem::ConstPtr;
use crate::objc::{id, msg, msg_class};
use crate::Environment;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreText.framework/CoreText",
    aliases: &[],
    class_exports: &[],
    function_exports: &[FUNCTIONS],
    constant_exports: &[],
};

pub type CTFontRef = id;

// REMOVED: #[no_mangle] and extern "C"
pub fn CTFontCreateWithGraphicsFont(
    env: &mut Environment,
    cg_font: CGFontRef,
    size: CGFloat,
    _transform: ConstPtr<CGAffineTransform>,
    _attributes: id,
) -> CTFontRef {
    log!("CTFontCreateWithGraphicsFont called, size={}", size);

    // Get the name from the CGFont handle
    let name = crate::frameworks::core_graphics::cg_font::CGFontCopyPostScriptName(env, cg_font);
    let font_name_str = ns_string::to_rust_string(env, name);
    
    let default_size: CGFloat = if size == 0.0 { 17.0 } else { size };
    let uifont_class = msg_class![env; UIFont class];
    
    let name_ns = ns_string::from_rust_string(env, font_name_str.to_string());
    msg![env; uifont_class fontWithName:name_ns size:default_size]
}

// REMOVED: #[no_mangle] and extern "C"
pub fn CTFontCopyGraphicsFont(
    _env: &mut Environment,
    _font: CTFontRef,
    _attributes: ConstPtr<id>,
) -> CGFontRef {
    log!("CTFontCopyGraphicsFont stub called");
    crate::objc::nil
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CTFontCreateWithGraphicsFont(_, _, _, _)),
    export_c_func!(CTFontCopyGraphicsFont(_, _)),
];
