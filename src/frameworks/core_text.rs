// src/frameworks/core_text.rs

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::cg_font::CGFontRef;
// FIX 1: Import CGAffineTransform from its specific submodule
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::CGFloat;
use crate::frameworks::foundation::ns_string;
use crate::mem::ConstPtr;
use crate::objc::{id, msg_class};
use crate::Environment;

pub type CTFontRef = id;

#[no_mangle]
pub extern "C" fn CTFontCreateWithGraphicsFont(
    env: &mut Environment,
    cg_font: CGFontRef,
    size: CGFloat,
    _transform: ConstPtr<CGAffineTransform>,
    _attributes: id,
) -> CTFontRef {
    let name = crate::frameworks::core_graphics::cg_font::CGFontCopyPostScriptName(env, cg_font);
    let font_name_str = crate::frameworks::foundation::ns_string::to_rust_string(env, name);
    
    let default_size: CGFloat = if size == 0.0 { 17.0 } else { size };

    // FIX 2: Convert Cow<str> to String using .to_string()
    let name_ns = ns_string::from_rust_string(env, font_name_str.to_string());
    
    msg_class![env; UIFont fontWithName:name_ns size:default_size]
}

pub const FUNCTIONS: FunctionExports = &[
    // FIX 3: Use exactly 4 underscores (the macro adds 'env' automatically)
    export_c_func!(CTFontCreateWithGraphicsFont(_, _, _, _)),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/CoreText.framework/CoreText",
    aliases: &[],
    class_exports: &[],
    // FIX 4: Wrap FUNCTIONS in an extra slice [&[...]] 
    function_exports: &[FUNCTIONS], 
    constant_exports: &[],
};
