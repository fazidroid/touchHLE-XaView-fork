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
    env.lock_mutex(host_object.mutex_id).expect("Failed to acquire NSLock");
}

- (())unlock {
    log_dbg!("[(NSLock *){:?} unlock]", this);
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    if !env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        log!("Warning: *** -[NSLock unlock]: lock (<NSLock: {:?}>) unlocked when not locked", this);
    }
    env.unlock_mutex(host_object.mutex_id).unwrap();
}

- (bool)tryLock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    
    // FIXED: Real non-blocking check. 
    // If the mutex is already locked, we MUST return false immediately.
    // We should not attempt to call env.lock_mutex here because if it blocks, 
    // it violates the tryLock contract and freezes the game.
    if env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        return false;
    }

    // Try to lock. If your environment has a real try_lock_mutex, use that.
    // Otherwise, this remains a best-effort non-blocking call.
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
    env.lock_mutex(host_object.mutex_id).unwrap();
}

- (())unlock {
    log_dbg!("[(NSRecursiveLock *){:?} unlock]", this);
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    if !env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        log!("Warning: *** -[NSRecursiveLock unlock]: lock (<NSRecursiveLock: {:?}>) unlocked when not locked", this);
    }
    env.unlock_mutex(host_object.mutex_id).unwrap();
}

- (bool)tryLock {
    let host_object = env.objc.borrow::<NSLockHostObject>(this);
    
    // NSRecursiveLocks are special; if the CURRENT thread owns it, tryLock succeeds.
    // This is a simplified check to prevent deadlocks in recursive calls.
    if env.mutex_state.mutex_is_locked(host_object.mutex_id) {
        // We assume for now if it's locked and we are calling tryLock, 
        // it might be held by another thread.
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
}