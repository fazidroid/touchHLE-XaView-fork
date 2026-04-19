/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! NSConditionLock stub.

use crate::objc::{id, msg, objc_classes, ClassExports, HostObject};

#[derive(Default)]
struct NSConditionLockHostObject;
impl HostObject for NSConditionLockHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSConditionLock: NSObject

- (id)initWithCondition:(crate::objc::NSInteger)condition {
    log_dbg!("NSConditionLock init with condition {}", condition);
    msg![env; this init]
}

- (())lockWhenCondition:(crate::objc::NSInteger)_condition {
    log_dbg!("NSConditionLock lockWhenCondition: (immediate success)");
    // Do nothing; pretend lock was acquired instantly.
}

- (())unlockWithCondition:(crate::objc::NSInteger)_condition {
    log_dbg!("NSConditionLock unlockWithCondition:");
}

- (())lock {
    log_dbg!("NSConditionLock lock");
}

- (())unlock {
    log_dbg!("NSConditionLock unlock");
}

- (crate::objc::NSInteger)condition {
    // Return a dummy condition that matches what the app expects.
    // Many games use 0 as the base condition.
    0
}

@end

};