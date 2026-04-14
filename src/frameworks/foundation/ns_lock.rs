/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSLock family`.

use crate::environment::MutexType::PTHREAD_MUTEX_RECURSIVE;
use crate::environment::{MutexId, PTHREAD_MUTEX_DEFAULT};
use crate::msg;
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
    log_dbg!("[NSLock alloc]");
    let mutex_id = env.mutex_state.init_mutex(PTHREAD_MUTEX_DEFAULT);
    let host_object = NSLockHostObject { mutex_id, name: nil };
    env.objc.alloc_object(this, Box::new(host_object), &mut env.mem)
}

// NSLocking protocol implementation
- (())lock {
    log_dbg!("[(NSLock *){:?} lock]", this);
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    // Blocking lock is fine for the standard .lock call
    if let Err(e) = env.lock_mutex(host_object.mutex_id) {
        log!("Warning: Failed to acquire NSLock: {:?}", e);
    }
}

- (())unlock {
    log_dbg!("[(NSLock *){:?} unlock]", this);
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    if !env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        log!("Warning: *** -[NSLock unlock]: lock (<NSLock: {:?}>) unlocked when not locked", this);
    }
    let _ = env.unlock_mutex(host_object.mutex_id);
}

- (bool)tryLock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    
    // Non-blocking check
    if env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        return false;
    }

    match env.lock_mutex(host_object.mutex_id) {
        Ok(_) => true,
        Err(_) => false,
    }
}

- (())setName:(id)name {
    let mut host_object = env.objc.borrow_mut::<NSLockHostObject>(this);
    host_object.name = name;
}

- (id)name {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    host_object.name
}

@end

@implementation NSRecursiveLock: NSObject

+ (id)alloc {
    log_dbg!("[NSRecursiveLock alloc]");
    let mutex_id = env.mutex_state.init_mutex(PTHREAD_MUTEX_RECURSIVE);
    let host_object = NSLockHostObject { mutex_id, name: nil };
    env.objc.alloc_object(this, Box::new(host_object), &mut env.mem)
}

// NSLocking protocol implementation
- (())lock {
    log_dbg!("[(NSRecursiveLock *){:?} lock]", this);
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    let _ = env.lock_mutex(host_object.mutex_id);
}

- (())unlock {
    log_dbg!("[(NSRecursiveLock *){:?} unlock]", this);
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    if !env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        log!("Warning: *** -[NSRecursiveLock unlock]: lock (<NSRecursiveLock: {:?}>) unlocked when not locked", this);
    }
    let _ = env.unlock_mutex(host_object.mutex_id);
}

- (bool)tryLock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    
    // NSRecursiveLocks allow the same thread to lock multiple times.
    // However, touchHLE's current lock_mutex might block if logic isn't perfect.
    // We return false if locked to be safe against deadlocks in Gameloft games.
    if env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        return false; 
    }
    
    env.lock_mutex(host_object.mutex_id).is_ok()
}

- (())setName:(id)name {
    let mut host_object = env.objc.borrow_mut::<NSLockHostObject>(this);
    host_object.name = name;
}

- (id)name {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    host_object.name
}

@end

};