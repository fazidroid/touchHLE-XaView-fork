/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIGraphics.h`

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::cg_context::{
    CGContextRef, CGContextRelease, CGContextRetain,
};
use crate::frameworks::core_graphics::CGSize;
use crate::objc::{id, msg, msg_class, nil};
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
    // Безопасно кладем nil в стек — функции CoreGraphics это поддерживают
    UIGraphicsPushContext(env, nil);
}

pub fn UIGraphicsGetImageFromCurrentImageContext(env: &mut Environment) -> id {
    log!("TODO: UIGraphicsGetImageFromCurrentImageContext");
    // Возвращаем пустую, но правильно инициализированную картинку
    let img: id = msg_class![env; UIImage alloc];
    msg![env; img init]
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
