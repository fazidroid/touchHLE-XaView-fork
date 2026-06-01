/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSDate`.

use super::ns_string::{from_rust_ordering, from_rust_string, get_static_str};
use crate::Environment;
use super::{NSComparisonResult, NSTimeInterval};
use crate::frameworks::core_foundation::time::{
    apple_epoch, CFAbsoluteTimeGetGregorianDate, SECS_FROM_UNIX_TO_APPLE_EPOCHS,
};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::frameworks::foundation::ns_keyed_unarchiver::decode_current_date;
use std::ops::{Add, Sub};
use std::time::{Duration, SystemTime};

#[derive(Default)]
pub(super) struct NSDateHostObject {
    pub(super) time_interval: NSTimeInterval,
}
impl HostObject for NSDateHostObject {}

// Helper to check if an object is really an NSDate
fn is_nsdate(env: &mut Environment, obj: id) -> bool {
    if obj == nil { return false; }
    // Use a direct Rust type check instead of sending isKindOfClass: via ObjC.
    // Sending a message requires walking the isa/superclass chain which calls
    // borrow::<ClassHostObject> — if obj is a zombie NSAutoreleasePool this
    // panics before we can return false. Checking the host object type directly
    // is safe and avoids any ObjC dispatch.
    env.objc
        .get_host_object(obj)
        .and_then(|h| h.as_any().downcast_ref::<NSDateHostObject>())
        .is_some()
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSDate: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSDateHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (NSTimeInterval)timeIntervalSinceReferenceDate {
    SystemTime::now()
        .duration_since(apple_epoch())
        .unwrap()
        .as_secs_f64()
}

+ (id)date {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    autorelease(env, new)
}

+ (id)distantFuture {
    let time_interval = SystemTime::now()
        .duration_since(apple_epoch())
        .unwrap()
        .as_secs_f64() * 2.0;
    let host_object = Box::new(NSDateHostObject { time_interval });
    let new = env.objc.alloc_object(this, host_object, &mut env.mem);
    autorelease(env, new)
}

+ (id)distantPast {
    let time_interval = -(SECS_FROM_UNIX_TO_APPLE_EPOCHS as f64);
    let host_object = Box::new(NSDateHostObject { time_interval });
    let new = env.objc.alloc_object(this, host_object, &mut env.mem);
    autorelease(env, new)
}

+ (id)dateWithTimeIntervalSinceNow:(NSTimeInterval)secs {
    let now: id = msg_class![env; NSDate date];
    msg![env; now addTimeInterval:secs]
}

+ (id)dateWithTimeIntervalSince1970:(NSTimeInterval)secs {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithTimeIntervalSince1970:secs];
    autorelease(env, new)
}

+ (id)dateWithTimeInterval:(NSTimeInterval)secs
                 sinceDate:(id)date {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithTimeInterval:secs sinceDate:date];
    autorelease(env, new)
}

+ (id)dateWithTimeIntervalSinceReferenceDate:(NSTimeInterval)secs {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithTimeIntervalSinceReferenceDate:secs];
    autorelease(env, new)
}

- (id)copyWithZone:(NSZonePtr)_zone {
    if !is_nsdate(env, this) { return nil; }
    retain(env, this)
}

- (id)init {
    if !is_nsdate(env, this) { return nil; }
    let time_interval = SystemTime::now()
        .duration_since(apple_epoch())
        .unwrap()
        .as_secs_f64();
    env.objc.borrow_mut::<NSDateHostObject>(this).time_interval = time_interval;
    this
}

- (id)initWithTimeInterval:(NSTimeInterval)secs
                 sinceDate:(id)date {
    if !is_nsdate(env, this) { return nil; }
    if !is_nsdate(env, date) { return nil; }
    let time_interval = env.objc.borrow_mut::<NSDateHostObject>(date).time_interval + secs;
    env.objc.borrow_mut::<NSDateHostObject>(this).time_interval = time_interval;
    this
}

- (id)initWithTimeIntervalSinceNow:(NSTimeInterval)secs {
    if !is_nsdate(env, this) { return nil; }
    let time_interval = SystemTime::now()
        .duration_since(apple_epoch())
        .unwrap()
        .as_secs_f64();
    env.objc.borrow_mut::<NSDateHostObject>(this).time_interval = time_interval + secs;
    this
}

- (id)initWithTimeIntervalSinceReferenceDate:(NSTimeInterval)secs {
    if !is_nsdate(env, this) { return nil; }
    env.objc.borrow_mut::<NSDateHostObject>(this).time_interval = secs;
    this
}

- (id)initWithTimeIntervalSince1970:(NSTimeInterval)secs {
    if !is_nsdate(env, this) { return nil; }
    let time_interval = -(SECS_FROM_UNIX_TO_APPLE_EPOCHS as f64) + secs;
    env.objc.borrow_mut::<NSDateHostObject>(this).time_interval = time_interval;
    this
}

- (id)initWithCoder:(id)coder {
    if !is_nsdate(env, this) { return nil; }
    release(env, this);
    decode_current_date(env, coder)
}

- (NSTimeInterval)timeIntervalSinceDate:(id)anotherDate {
    if !is_nsdate(env, this) { return 0.0; }
    if !is_nsdate(env, anotherDate) { return 0.0; }
    let host_object = env.objc.borrow::<NSDateHostObject>(this);
    let another_date_host_object = env.objc.borrow::<NSDateHostObject>(anotherDate);
    host_object.time_interval - another_date_host_object.time_interval
}

- (NSTimeInterval)timeIntervalSinceReferenceDate {
    if !is_nsdate(env, this) { return 0.0; }
    env.objc.borrow::<NSDateHostObject>(this).time_interval
}

- (NSTimeInterval)timeIntervalSinceNow {
    if !is_nsdate(env, this) { return 0.0; }
    let host_object = env.objc.borrow::<NSDateHostObject>(this);
    let time_interval = SystemTime::now()
        .duration_since(apple_epoch())
        .unwrap()
        .as_secs_f64();
    time_interval - host_object.time_interval
}

- (NSTimeInterval)timeIntervalSince1970 {
    if !is_nsdate(env, this) { return 0.0; }
    let time_interval = env.objc.borrow::<NSDateHostObject>(this).time_interval;
    let new_time = if time_interval >= 0.0 {
        apple_epoch().add(Duration::from_secs_f64(time_interval))
    } else {
        apple_epoch().sub(Duration::from_secs_f64(-time_interval))
    };
    new_time.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs_f64()
}

- (id)addTimeInterval:(NSTimeInterval)seconds {
    if !is_nsdate(env, this) { return nil; }
    let interval = env.objc.borrow::<NSDateHostObject>(this).time_interval + seconds;
    let date = msg_class![env; NSDate date];
    env.objc.borrow_mut::<NSDateHostObject>(date).time_interval = interval;
    date
}

- (NSComparisonResult)compare:(id)anotherDate {
    if !is_nsdate(env, this) { return 0; }
    if !is_nsdate(env, anotherDate) { return 0; }
    let host_object = env.objc.borrow::<NSDateHostObject>(this);
    let another_date_host_object = env.objc.borrow::<NSDateHostObject>(anotherDate);
    from_rust_ordering(host_object.time_interval.total_cmp(&another_date_host_object.time_interval))
}

- (id)description {
    if !is_nsdate(env, this) { return get_static_str(env, ""); }
    let time_interval = env.objc.borrow::<NSDateHostObject>(this).time_interval;
    let greg_date = CFAbsoluteTimeGetGregorianDate(env, time_interval, nil);
    // Copy fields to local variables to avoid unaligned reference
    let year = greg_date.year;
    let month = greg_date.month;
    let day = greg_date.day;
    let hours = greg_date.hours;
    let minutes = greg_date.minutes;
    let seconds = greg_date.seconds;
    let desc = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} +0000",
        year, month, day, hours, minutes, seconds as i32
    );
    let desc_string = from_rust_string(env, desc);
    autorelease(env, desc_string)
}

@end

};
