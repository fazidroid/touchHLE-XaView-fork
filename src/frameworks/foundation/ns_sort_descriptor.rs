/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSSortDescriptor` implementation.

use crate::objc::{id, msg_class, msg_super, objc_classes, ClassExports, HostObject, NSZonePtr};

// MODIFIED: A struct to hold the actual sort descriptor data
#[derive(Debug)]
struct NSSortDescriptorHostObject {
    key: Option<id>,           // NSString
    ascending: bool,
    selector: Option<id>,      // SEL as NSString (or id)
}

impl HostObject for NSSortDescriptorHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSSortDescriptor: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    // MODIFIED: Allocate with the proper struct, initializing fields to None/false
    let host_object = Box::new(NSSortDescriptorHostObject {
        key: None,
        ascending: true,
        selector: None,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)sortDescriptorWithKey:(id)key ascending:(bool)ascending {
    log_dbg!("NSSortDescriptor sortDescriptorWithKey:ascending:");
    let desc: id = msg_class![env; NSSortDescriptor alloc];
    let desc: id = msg![env; desc initWithKey:key ascending:ascending];
    // Note: TouchHLE typically uses autorelease, but new/alloc pattern returns retained object.
    // This is fine as long as the caller knows to release.
    desc
}

+ (id)sortDescriptorWithKey:(id)key ascending:(bool)ascending selector:(id)selector {
    log_dbg!("NSSortDescriptor sortDescriptorWithKey:ascending:selector:");
    let desc: id = msg_class![env; NSSortDescriptor alloc];
    let desc: id = msg![env; desc initWithKey:key ascending:ascending selector:selector];
    desc
}

- (id)initWithKey:(id)key ascending:(bool)ascending {
    log_dbg!("NSSortDescriptor initWithKey:ascending:");
    // MODIFIED: Store the passed values
    let host_object = env.objc.borrow_mut::<NSSortDescriptorHostObject>(this);
    host_object.key = Some(key);
    host_object.ascending = ascending;
    host_object.selector = None;
    this
}

- (id)initWithKey:(id)key ascending:(bool)ascending selector:(id)selector {
    log_dbg!("NSSortDescriptor initWithKey:ascending:selector:");
    // MODIFIED: Store the passed values including selector
    let host_object = env.objc.borrow_mut::<NSSortDescriptorHostObject>(this);
    host_object.key = Some(key);
    host_object.ascending = ascending;
    host_object.selector = Some(selector);
    this
}

- (id)key {
    // MODIFIED: Return the stored key or nil if not set
    env.objc.borrow::<NSSortDescriptorHostObject>(this).key.unwrap_or(0 as id)
}

- (bool)ascending {
    // MODIFIED: Return the stored ascending flag
    env.objc.borrow::<NSSortDescriptorHostObject>(this).ascending
}

- (id)selector {
    // MODIFIED: Return the stored selector or nil
    env.objc.borrow::<NSSortDescriptorHostObject>(this).selector.unwrap_or(0 as id)
}

@end

};