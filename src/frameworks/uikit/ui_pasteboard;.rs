/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIPasteboard` stub.

use crate::objc::{objc_classes, ClassExports, HostObject};
use crate::Environment;

#[derive(Default)]
struct UIPasteboardHostObject;
impl HostObject for UIPasteboardHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIPasteboard: NSObject

+ (id)generalPasteboard {
    let cls = env.objc.get_known_class("UIPasteboard", &mut env.mem);
    env.objc.alloc_static_object(cls, Box::new(UIPasteboardHostObject), &mut env.mem)
}

- (())setPersistent:(bool)persistent {
    log!("UIPasteboard setPersistent: {} stub called", persistent);
}

@end

};