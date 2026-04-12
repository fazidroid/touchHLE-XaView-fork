/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! `SKPaymentQueue`
//!
//! Fix: `defaultQueue` previously returned `nil`. The game stores this object
//! and calls `addTransactionObserver:` on it. A nil return caused a silent
//! crash/hang. Now returns a real singleton instance.

use crate::objc::{id, nil, objc_classes, retain, ClassExports, HostObject};

/// Per-instance state for SKPaymentQueue.
struct SKPaymentQueueHostObject {
    /// Weak reference to the registered transaction observer (if any).
    observer: id,
}
impl HostObject for SKPaymentQueueHostObject {}

/// Global singleton pointer. touchHLE runs one app at a time so this is safe.
static mut SK_PAYMENT_QUEUE_SINGLETON: id = nil;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation SKPaymentQueue: NSObject

+ (id)defaultQueue {
    // Return a real singleton so callers don't crash dereferencing nil.
    let existing = unsafe { SK_PAYMENT_QUEUE_SINGLETON };
    if existing != nil {
        return existing;
    }
    let host_object = Box::new(SKPaymentQueueHostObject { observer: nil });
    let instance = env.objc.alloc_object(this, host_object, &mut env.mem);
    unsafe { SK_PAYMENT_QUEUE_SINGLETON = instance };
    instance
}

+ (bool)canMakePayments {
    // Returning false is the fastest path — the game's CRM task will treat
    // IAP as unavailable and skip waiting for purchase results.
    false
}

- (())addTransactionObserver:(id)observer {
    // Store the observer (weak ref — the game owns it).
    env.objc.borrow_mut::<SKPaymentQueueHostObject>(this).observer = observer;
}

- (())removeTransactionObserver:(id)_observer {
    env.objc.borrow_mut::<SKPaymentQueueHostObject>(this).observer = nil;
}

- (())addPayment:(id)_payment {
    // No-op: no real store connectivity.
}

- (())restoreCompletedTransactions {
    // No-op: no transactions to restore.
}

@end

};
