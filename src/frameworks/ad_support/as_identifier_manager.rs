/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/ .
 */
//! `ASIdentifierManager` class from the AdSupport framework.

use crate::objc::{id, nil, objc_classes, ClassExports, HostObject, msg, msg_class};
use std::cell::Cell;

// Singleton storage. touchHLE runs single-threaded, so thread_local is safe.
thread_local! {
    static SHARED_MANAGER: Cell<id> = Cell::new(nil);
}

// Host object for `ASIdentifierManager` instances.
pub struct ASIdentifierManagerHostObject {
    pub advertising_identifier: id,
}
impl HostObject for ASIdentifierManagerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation ASIdentifierManager: NSObject

    // Returns the shared singleton instance.
    + (id)sharedManager {
        let existing = SHARED_MANAGER.with(|c| c.get());
        if existing != nil {
            return existing;
        }
        let instance: id = msg_class![env; ASIdentifierManager alloc];
        let instance: id = msg![env; instance init];
        SHARED_MANAGER.with(|c| c.set(instance));
        instance
    }

    - (id)init {
        env.objc.borrow_mut::<ASIdentifierManagerHostObject>(this)
            .advertising_identifier = nil;
        this
    }

    // Returns the advertising identifier (IDFA) as an NSUUID.
    - (id)advertisingIdentifier {
        env.objc
            .borrow::<ASIdentifierManagerHostObject>(this)
            .advertising_identifier
    }

    // Returns whether ad tracking is enabled.
    - (bool)isAdvertisingTrackingEnabled {
        false
    }

    @end
};
