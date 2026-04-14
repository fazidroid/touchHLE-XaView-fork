/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSTimeZone`.

use crate::frameworks::foundation::{ns_string, NSInteger};
use crate::objc::{autorelease, id, nil, release, retain, ClassExports, HostObject, NSZonePtr};
use crate::{msg, objc_classes};

struct NSTimeZoneHostObject {
    // NSString*
    time_zone: id,
}
impl HostObject for NSTimeZoneHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSTimeZone: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSTimeZoneHostObject {
        time_zone: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)timeZoneWithName:(id)tz_name {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithName:tz_name];
    autorelease(env, new)
}

+ (id)localTimeZone {
    // As reported by the Aspen Simulator
    let tz_name: id = ns_string::get_static_str(env, "Canada/Eastern");
    msg![env; this timeZoneWithName:tz_name]
}

- (())dealloc {
    let tz_name = env.objc.borrow_mut::<NSTimeZoneHostObject>(this).time_zone;
    release(env, tz_name);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)initWithName:(id)tz_name { // NSString *
    let tz = if tz_name == nil {
        // fallback to UTC if nil
        ns_string::get_static_str(env, "UTC")
    } else {
        tz_name
    };

    retain(env, tz);
    env.objc.borrow_mut::<NSTimeZoneHostObject>(this).time_zone = tz;
    this
}

- (id)name {
    env.objc.borrow_mut::<NSTimeZoneHostObject>(this).time_zone
}

- (NSInteger)secondsFromGMT {
    // TODO: respect timezone
    0
}

@end

};
