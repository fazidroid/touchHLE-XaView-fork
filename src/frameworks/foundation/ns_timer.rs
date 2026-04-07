/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSTimer`.

use super::ns_run_loop::NSDefaultRunLoopMode;
use super::NSTimeInterval;
use super::{ns_run_loop, ns_string};
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, nil, objc_classes, release, retain, ClassExports,
    HostObject, SEL,
};
use crate::Environment;
use std::time::{Duration, Instant};

struct NSTimerHostObject {
    ns_interval: NSTimeInterval,
    /// Copy of `ns_interval` in Rust's type for time intervals. Keep in sync!
    rust_interval: Duration,
    /// Strong reference
    target: id,
    selector: SEL,
    /// Strong reference
    user_info: id,
    repeats: bool,
    due_by: Option<Instant>,
    /// If the timer is currently running its callback, this is set so that the
    /// re-entering the run loop from inside the callback doesn't cause an
    /// infinite loop.
    is_running_callback: bool,
    /// Weak reference
    run_loop: id,
}
impl HostObject for NSTimerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSTimer: NSObject

+ (id)timerWithTimeInterval:(NSTimeInterval)interval
                     target:(id)target
                   selector:(SEL)selector
                   userInfo:(id)userInfo
                    repeats:(bool)repeats {
    let new: id = msg_class![env; NSTimer alloc];

    // GAMELOFT NaN FIX: Prevent crashes when delta time is NaN or 0
    let safe_interval = if interval.is_finite() && interval > 0.0 { interval } else { 1.0 };
    let rust_interval = Duration::from_secs_f64(safe_interval);

    let host_object = Box::new(NSTimerHostObject {
        ns_interval: interval,
        rust_interval,
        target,
        selector,
        user_info: userInfo,
        repeats,
        due_by: None,
        is_running_callback: false,
        run_loop: nil,
    });
    retain(env, target);
    retain(env, userInfo);
    let new = env.objc.alloc_object(new, host_object, &mut env.mem);
    autorelease(env, new)
}

+ (id)scheduledTimerWithTimeInterval:(NSTimeInterval)interval
                              target:(id)target
                            selector:(SEL)selector
                            userInfo:(id)userInfo
                             repeats:(bool)repeats {
    let new: id = msg_class![env; NSTimer timerWithTimeInterval:interval
                                                         target:target
                                                       selector:selector
                                                       userInfo:userInfo
                                                        repeats:repeats];
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    let mode = ns_string::get_static_str(env, ns_run_loop::NSDefaultRunLoopMode);
    () = msg![env; run_loop addTimer:new forMode:mode];
    new
}

- (id)initWithFireDate:(id)date
              interval:(NSTimeInterval)interval
                target:(id)target
              selector:(SEL)selector
              userInfo:(id)userInfo
               repeats:(bool)repeats {
    // GAMELOFT NaN FIX: Prevent crashes when delta time is NaN or 0
    let safe_interval = if interval.is_finite() && interval > 0.0 { interval } else { 1.0 };
    let rust_interval = Duration::from_secs_f64(safe_interval);

    let host_object = Box::new(NSTimerHostObject {
        ns_interval: interval,
        rust_interval,
        target,
        selector,
        user_info: userInfo,
        repeats,
        due_by: None,
        is_running_callback: false,
        run_loop: nil,
    });
    retain(env, target);
    retain(env, userInfo);
    let this = env.objc.alloc_object(this, host_object, &mut env.mem);

    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    let mode = ns_string::get_static_str(env, ns_run_loop::NSDefaultRunLoopMode);
    () = msg![env; run_loop addTimer:this forMode:mode];

    // TODO: Actually use `date`

    this
}

- (bool)isValid {
    let host_object = env.objc.borrow::<NSTimerHostObject>(this);
    host_object.due_by.is_some()
}

- (())invalidate {
    let host_object = env.objc.borrow_mut::<NSTimerHostObject>(this);
    host_object.due_by = None;
    if host_object.run_loop != nil {
        ns_run_loop::remove_timer(env, host_object.run_loop, this);
        host_object.run_loop = nil;
    }
}

- (())dealloc {
    let host_object = env.objc.borrow::<NSTimerHostObject>(this);
    if host_object.run_loop != nil {
        ns_run_loop::remove_timer(env, host_object.run_loop, this);
    }
    release(env, host_object.target);
    release(env, host_object.user_info);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};

pub fn schedule(env: &mut Environment, timer: id, run_loop: id) {
    let host_object = env.objc.borrow_mut::<NSTimerHostObject>(timer);
    if host_object.due_by.is_none() {
        // Note: This matches the behaviour of `scheduledTimerWithTimeInterval` on iOS 2.0
        host_object.due_by = Some(Instant::now() + host_object.rust_interval);
    }
    host_object.run_loop = run_loop;
}

pub fn due_by(env: &mut Environment, timer: id) -> Option<Instant> {
    let host_object = env.objc.borrow::<NSTimerHostObject>(timer);
    host_object.due_by
}

pub fn fire(env: &mut Environment, timer: id, run_loop: id) {
    let host_object = env.objc.borrow::<NSTimerHostObject>(timer);
    if host_object.is_running_callback || host_object.due_by.is_none() {
        return;
    }
    let target = host_object.target;
    let selector = host_object.selector;
    let repeats = host_object.repeats;
    let rust_interval = host_object.rust_interval;
    let ns_interval = host_object.ns_interval;
    let due_by = host_object.due_by.unwrap();

    let new_due_by = if repeats {
        let overdue_by = Instant::now().saturating_duration_since(due_by);
        
        // GAMELOFT NaN FIX: Sanitize the division to prevent panics!
        let safe_ns_interval = if ns_interval.is_finite() && ns_interval > 0.0 { ns_interval } else { 1.0 };
        let advance_by = (overdue_by.as_secs_f64() / safe_ns_interval).max(1.0).ceil();
        assert!(advance_by == (advance_by as u32) as f64);
        
        let advance_by = advance_by as u32;
        if advance_by > 1 {
            log_dbg!("Warning: Timer {:?} is lagging. It is overdue by {}s and has missed {} interval(s)!", timer, overdue_by.as_secs_f64(), advance_by - 1);
        }
        let advance_by = rust_interval.checked_mul(advance_by).unwrap();
        Some(due_by.checked_add(advance_by).unwrap())
    } else {
        ns_run_loop::remove_timer(env, run_loop, timer);
        None
    };
    env.objc.borrow_mut::<NSTimerHostObject>(timer).due_by = new_due_by;
    env.objc
        .borrow_mut::<NSTimerHostObject>(timer)
        .is_running_callback = true;

    log_dbg!(
        "Timer {:?} fired, sending {:?} message to {:?}",
        timer,
        selector.as_str(&env.mem),
        target
    );

    let pool: id = msg_class![env; NSAutoreleasePool new];
    () = msg_send!(env, (target, selector, timer));
    release(env, pool);

    env.objc
        .borrow_mut::<NSTimerHostObject>(timer)
        .is_running_callback = false;
}