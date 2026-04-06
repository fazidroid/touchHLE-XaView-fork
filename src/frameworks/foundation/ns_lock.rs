/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSLock family`.

use crate::environment::MutexType::PTHREAD_MUTEX_RECURSIVE;
use crate::environment::{MutexId, PTHREAD_MUTEX_DEFAULT};
use crate::objc::{id, nil, objc_classes, ClassExports, HostObject};

struct NSLockHostObject {
    mutex_id: MutexId,
    name: id,
}
impl HostObject for NSLockHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSLock: NSObject

+ (id)alloc {
    let mutex_id = env.mutex_state.init_mutex(PTHREAD_MUTEX_DEFAULT);
    let host_object = NSLockHostObject { mutex_id, name: nil };
    env.objc.alloc_object(this, Box::new(host_object), &mut env.mem)
}

- (())lock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    let _ = env.lock_mutex(host_object.mutex_id);
}

- (())unlock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    let _ = env.unlock_mutex(host_object.mutex_id);
}

- (bool)tryLock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    if env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        return false;
    }
    env.lock_mutex(host_object.mutex_id).is_ok()
}

- (())setName:(id)name {
    env.objc.borrow_mut::<NSLockHostObject>(this).name = name;
}

- (id)name {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    host_object.name
}

@end

@implementation NSRecursiveLock: NSObject

+ (id)alloc {
    let mutex_id = env.mutex_state.init_mutex(PTHREAD_MUTEX_RECURSIVE);
    let host_object = NSLockHostObject { mutex_id, name: nil };
    env.objc.alloc_object(this, Box::new(host_object), &mut env.mem)
}

- (())lock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    let _ = env.lock_mutex(host_object.mutex_id);
}

- (())unlock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    let _ = env.unlock_mutex(host_object.mutex_id);
}

- (bool)tryLock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    if env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        return false; 
    }
    env.lock_mutex(host_object.mutex_id).is_ok()
}

- (())setName:(id)name {
    env.objc.borrow_mut::<NSLockHostObject>(this).name = name;
}

- (id)name {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    host_object.name
}

@end

};