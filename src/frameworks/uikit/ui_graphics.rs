/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIGraphics.h`

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::cg_context::{
    CGContextRef, CGContextRelease, CGContextRetain, CGContextHostObject, CGContextSubclass
};
use crate::frameworks::core_graphics::cg_bitmap_context::CGBitmapContextData;
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::CGSize;
use crate::objc::{id, nil, msg_class};
use crate::mem::Ptr;
use crate::Environment;

#[derive(Default)]
pub(super) struct State {
    pub(super) context_stack: Vec<CGContextRef>,
}

pub fn UIGraphicsPushContext(env: &mut Environment, context: CGContextRef) {
    CGContextRetain(env, context);
    env.framework_state
        .uikit
        .ui_graphics
        .context_stack
        .push(context);
}
pub fn UIGraphicsPopContext(env: &mut Environment) {
    if let Some(context) = env.framework_state.uikit.ui_graphics.context_stack.pop() {
        CGContextRelease(env, context);
    }
}
pub fn UIGraphicsGetCurrentContext(env: &mut Environment) -> CGContextRef {
    env.framework_state
        .uikit
        .ui_graphics
        .context_stack
        .last()
        .copied()
        .unwrap_or(nil)
}

pub fn UIGraphicsBeginImageContext(env: &mut Environment, size: CGSize) {
    log!("TODO: UIGraphicsBeginImageContext size {:?}", size);
    
    let class = env.objc.get_known_class("_touchHLE_CGContext", &mut env.mem);
    let host_object = Box::new(CGContextHostObject {
        subclass: CGContextSubclass::CGBitmapContext(CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: size.width as usize,
            height: size.height as usize,
            bits_per_component: 8,
            bytes_per_row: (size.width * 4.0) as usize,
            color_space: nil,
            bitmap_info: 0,
        }),
        rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
        transform: CGAffineTransform::identity(),
        state_stack: Vec::new(),
    });
    
    let context = env.objc.alloc_object(class, host_object, &mut env.mem);
    UIGraphicsPushContext(env, context);
    // Remove extra +1 retain count from alloc, so it dies properly on pop
    CGContextRelease(env, context);
}

pub fn UIGraphicsGetImageFromCurrentImageContext(env: &mut Environment) -> id {
    log!("TODO: UIGraphicsGetImageFromCurrentImageContext");
    msg_class![env; UIImage alloc]
}

pub fn UIGraphicsEndImageContext(env: &mut Environment) {
    log!("TODO: UIGraphicsEndImageContext");
    UIGraphicsPopContext(env);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(UIGraphicsPushContext(_)),
    export_c_func!(UIGraphicsPopContext()),
    export_c_func!(UIGraphicsGetCurrentContext()),
    export_c_func!(UIGraphicsBeginImageContext(_)),
    export_c_func!(UIGraphicsGetImageFromCurrentImageContext()),
    export_c_func!(UIGraphicsEndImageContext()),
];