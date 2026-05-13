// src/frameworks/core_text/mod.rs
//! Stubs and implementations for the Core Text framework.

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::cg_font::CGFontRef;
use crate::frameworks::core_graphics::{CGAffineTransform, CGFloat};
use crate::mem::ConstPtr;
use crate::objc::{id, msg_class};
use crate::Environment;
use crate::dyld::{export_c_func, export_c_func_aliased, FunctionExports};

/// Opaque type representing a Core Text font object.
/// CTFontRef is toll‑free bridged with UIFont.
pub type CTFontRef = id;

/// Core Text function to create a CTFont object from a CGFont.
/// The implementation uses the toll‑free bridge to UIFont.
///
/// * `cg_font`: The Core Graphics font to convert.
/// * `size`: The desired point size (if zero, a default size is used).
/// * `transform`: An affine transform to apply to the font (can be ignored for now).
/// * `attributes`: A font descriptor with additional attributes (can be ignored for now).
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
    let font_name_str =
        crate::frameworks::foundation::ns_string::to_rust_string(env, name);
    log_dbg!("... postscript name: {}", font_name_str);

    // Use the determined font name and size to create a UIFont.
    let default_size: CGFloat = if size == 0.0 { 17.0 } else { size };

    // UIFont is toll‑free bridged with CTFontRef.
    let class_uifont = msg_class![env; UIFont];
    let font: id = if !font_name_str.is_empty() {
        msg_class![env; UIFont fontWithName: crate::frameworks::foundation::ns_string::from_rust_string(env, font_name_str) size:default_size]
    } else {
        msg_class![env; UIFont systemFontOfSize:default_size]
    };

    // Securely pass the font object to the game.
    font
}

/// Helper function to get the name of a CGFont.
pub fn CGFontCopyPostScriptName(env: &mut Environment, font: CGFontRef) -> id {
    // This is a placeholder that always returns "Helvetica".
    // A proper implementation would need to read the actual font name.
    crate::frameworks::foundation::ns_string::from_rust_string(env, "Helvetica".to_string())
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CTFontCreateWithGraphicsFont(_, _, _, _, _)),
    export_c_func_aliased!("_CTFontCreateWithGraphicsFont", CTFontCreateWithGraphicsFont(_, _, _, _, _)),
];
