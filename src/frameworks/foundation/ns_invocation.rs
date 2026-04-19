/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSInvocation` stub.

use crate::frameworks::foundation::NSUInteger;
use crate::objc::{id, msg_class, msg_super, objc_classes, ClassExports, HostObject};

#[derive(Default)]
struct NSInvocationHostObject;
impl HostObject for NSInvocationHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSInvocation: NSObject

+ (id)invocationWithMethodSignature:(id)_signature {
    log_dbg!("NSInvocation invocationWithMethodSignature: stub");
    msg_class![env; NSInvocation new]
}

- (id)initWithMethodSignature:(id)_signature {
    log_dbg!("NSInvocation initWithMethodSignature: stub");
    msg_super![env; this init]
}

- (())setTarget:(id)_target {
    // ignore
}

- (())setSelector:(id)_selector {
    // ignore
}

- (())setArgument:(id)_argument atIndex:(NSUInteger)_index {
    // ignore
}

- (())invoke {
    log_dbg!("NSInvocation invoke (no-op)");
}

- (())invokeWithTarget:(id)_target {
    log_dbg!("NSInvocation invokeWithTarget: (no-op)");
}

- (())getReturnValue:(crate::mem::MutVoidPtr)_buffer {
    // Return a default value (e.g., 0 for scalar) - nothing to write for void returns usually
}

@end

};
