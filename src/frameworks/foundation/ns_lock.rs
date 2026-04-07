/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSLock` family.

use crate::environment::MutexType::PTHREAD_MUTEX_RECURSIVE;
use crate::environment::{MutexId, PTHREAD_MUTEX_DEFAULT};
use crate::objc::{id, nil, objc_classes, ClassExports, HostObject};

struct NSLockHostObject {
    mutex_id: MutexId,
    name: id,
}
impl HostObject for NSLockHostObject {}

struct NSConditionLockHostObject {
    mutex_id: MutexId,
    condition: i32,
}
impl HostObject for NSConditionLockHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSCitionLock: NSObject
+ (id)alloc {
    let mutex_id = env.mutex_state.init_mutex(PTHREAD_MUTEX_DEFAULT);
    let host_object = NSConditionLockHostObject { mutex_id, condition: 0 };
    env.objc.alloc_object(this, Box::new(host_object), &mut env.mem)
}
- (id)initWithCondition:(i32)condition {
    let host_obj = env.objc.borrow_mut::<NSConditionLockHostObject>(this);
    host_obj.condition = condition;
    this
}
- (i32)condition {
    env.objc.borrow::<NSConditionLockHostObject>(this).condition
}
- (())lock {
    let mutex_id = env.objc.borrow::<NSConditionLockHostObject>(this).mutex_id;
    let _ = env.lock_mutex(mutex_id);
}
- (())unlock {
    let mutex_id = env.objc.borrow::<NSConditionLockHostObject>(this).mutex_id;
    let _ = env.unlock_mutex(mutex_id);
}
- (())lockWhenCondition:(i32)_condition {
    let mutex_id = env.objc.borrow::<NSConditionLockHostObject>(this).mutex_id;
    let _ = env.lock_mutex(mutex_id);
}
- (())unlockWithCondition:(i32)condition {
    // FIXED: Copy the mutex_id first so we can drop the borrow of host_obj
    let mutex_id = {
        let host_obj = env.objc.borrow_mut::<NSConditionLockHostObject>(this);
        host_obj.condition = condition;
        host_obj.mutex_id
    }; // The borrow ends here
    
    let _ = env.unlock_mutex(mutex_id);
}
@end

};
