/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSSortDescriptor` implementation.

use crate::objc::{id, msg, msg_class, msg_super, nil, objc_classes, ClassExports, HostObject, NSZonePtr};

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
    let host_object = env.objc.borrow_mut::<NSSortDescriptorHostObject>(this);
    host_object.key = Some(key);
    host_object.ascending = ascending;
    host_object.selector = None;
    this
}

- (id)initWithKey:(id)key ascending:(bool)ascending selector:(id)selector {
    log_dbg!("NSSortDescriptor initWithKey:ascending:selector:");
    let host_object = env.objc.borrow_mut::<NSSortDescriptorHostObject>(this);
    host_object.key = Some(key);
    host_object.ascending = ascending;
    host_object.selector = Some(selector);
    this
}

- (id)key {
    env.objc.borrow::<NSSortDescriptorHostObject>(this).key.unwrap_or(nil)
}

- (bool)ascending {
    env.objc.borrow::<NSSortDescriptorHostObject>(this).ascending
}

- (id)selector {
    env.objc.borrow::<NSSortDescriptorHostObject>(this).selector.unwrap_or(nil)
}

@end

};