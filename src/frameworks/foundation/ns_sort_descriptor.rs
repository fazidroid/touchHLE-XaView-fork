/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSSortDescriptor` stub.

use crate::objc::{id, msg, msg_class, objc_classes, ClassExports, HostObject};

#[derive(Default)]
struct NSSortDescriptorHostObject;
impl HostObject for NSSortDescriptorHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSSortDescriptor: NSObject

+ (id)sortDescriptorWithKey:(id)_key ascending:(bool)_ascending {
    log_dbg!("NSSortDescriptor sortDescriptorWithKey:ascending: stub");
    msg_class![env; NSSortDescriptor new]
}

+ (id)sortDescriptorWithKey:(id)_key ascending:(bool)_ascending selector:(id)_selector {
    log_dbg!("NSSortDescriptor sortDescriptorWithKey:ascending:selector: stub");
    msg_class![env; NSSortDescriptor new]
}

- (id)initWithKey:(id)_key ascending:(bool)_ascending {
    log_dbg!("NSSortDescriptor initWithKey:ascending: stub");
    msg![env; this init]
}

- (id)initWithKey:(id)_key ascending:(bool)_ascending selector:(id)_selector {
    log_dbg!("NSSortDescriptor initWithKey:ascending:selector: stub");
    msg![env; this init]
}

- (id)key {
    // Return a dummy key to avoid nil
    crate::frameworks::foundation::ns_string::from_rust_string(env, "stubKey")
}

- (bool)ascending {
    true
}

- (id)selector {
    // Return a dummy selector name
    crate::frameworks::foundation::ns_string::from_rust_string(env, "compare:")
}

@end

};