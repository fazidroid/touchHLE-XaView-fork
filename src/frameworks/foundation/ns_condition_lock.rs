/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSConditionLock` stub to prevent thread deadlocks.

use crate::frameworks::foundation::NSInteger;
use crate::objc::{id, msg, msg_super, objc_classes, ClassExports, HostObject};

#[derive(Default)]
struct NSConditionLockHostObject {
    condition: NSInteger,
}
impl HostObject for NSConditionLockHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSConditionLock: NSObject

- (id)initWithCondition:(NSInteger)condition {
    log_dbg!("NSConditionLock initWithCondition: {}", condition);
    let host_obj = env.objc.borrow_mut::<NSConditionLockHostObject>(this);
    host_obj.condition = condition;
    msg_super![env; this init]
}

- (id)init {
    let host_obj = env.objc.borrow_mut::<NSConditionLockHostObject>(this);
    host_obj.condition = 0;
    msg_super![env; this init]
}

- (())lockWhenCondition:(NSInteger)condition {
    log_dbg!("NSConditionLock lockWhenCondition: {} (immediate success)", condition);
    let host_obj = env.objc.borrow_mut::<NSConditionLockHostObject>(this);
    host_obj.condition = condition;
}

- (())unlockWithCondition:(NSInteger)condition {
    log_dbg!("NSConditionLock unlockWithCondition: {}", condition);
    let host_obj = env.objc.borrow_mut::<NSConditionLockHostObject>(this);
    host_obj.condition = condition;
}

- (())lock {
    log_dbg!("NSConditionLock lock");
}

- (())unlock {
    log_dbg!("NSConditionLock unlock");
}

- (NSInteger)condition {
    let host_obj = env.objc.borrow::<NSConditionLockHostObject>(this);
    host_obj.condition
}

- (bool)tryLock {
    log_dbg!("NSConditionLock tryLock -> true");
    true
}

- (bool)tryLockWhenCondition:(NSInteger)condition {
    log_dbg!("NSConditionLock tryLockWhenCondition: {} -> true", condition);
    let host_obj = env.objc.borrow_mut::<NSConditionLockHostObject>(this);
    host_obj.condition = condition;
    true
}

- (())lockBeforeDate:(id)_limit {
    log_dbg!("NSConditionLock lockBeforeDate: (immediate success)");
}

- (id)name {
    crate::objc::nil
}

- (())setName:(id)_name {
    // ignore
}

@end

};
