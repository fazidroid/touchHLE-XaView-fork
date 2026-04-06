/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSOperation` and `NSOperationQueue`.

use crate::objc::{id, msg, nil, objc_classes, SEL, ClassExports, HostObject};
use crate::frameworks::foundation::NSInteger;
use crate::mem::Ptr;
use crate::Environment;

// Define host objects to satisfy the HostObject trait requirement
pub(super) struct NSOperationQueueHostObject;
impl HostObject for NSOperationQueueHostObject {}

pub(super) struct NSOperationHostObject;
impl HostObject for NSOperationHostObject {}

pub(super) struct NSInvocationOperationHostObject {
    pub target: id,
    pub sel: SEL,
    pub arg: id,
}
impl HostObject for NSInvocationOperationHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSOperationQueue: NSObject

+ (id)alloc {
    // FIXED: Use a proper HostObject struct instead of ()
    env.objc.alloc_object(this, Box::new(NSOperationQueueHostObject), &mut env.mem)
}

- (id)init { this }

- (())addOperation:(id)op {
    log_dbg!("[(NSOperationQueue*){:?} addOperation:{:?}] (executing synchronously!)", this, op);
    () = msg![env; op start];
}

// FIXED: Use NSInteger instead of isize to satisfy GuestArg trait
- (())setMaxConcurrentOperationCount:(NSInteger)_count { }

@end


@implementation NSOperation: NSObject

+ (id)alloc {
    // FIXED: Use a proper HostObject struct instead of ()
    env.objc.alloc_object(this, Box::new(NSOperationHostObject), &mut env.mem)
}

- (id)init { this }

- (())start {
    () = msg![env; this main];
}

- (())main { }

- (bool)isCancelled { false }

- (())cancel { }

@end


@implementation NSInvocationOperation: NSOperation

+ (id)alloc {
    let host_object = Box::new(NSInvocationOperationHostObject {
        target: nil,
        // FIXED: Initializing with a null pointer since from_raw was missing
        sel: SEL::from(Ptr::null()),
        arg: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithTarget:(id)target selector:(SEL)sel object:(id)arg {
    let host_object = env.objc.borrow_mut::<NSInvocationOperationHostObject>(this);
    host_object.target = target;
    host_object.sel = sel;
    host_object.arg = arg;
    this
}

- (())main {
    let host_object = env.objc.borrow::<NSInvocationOperationHostObject>(this);
    let target = host_object.target;
    let sel = host_object.sel;
    let arg = host_object.arg;
    
    if arg != nil {
        () = msg![env; target performSelector:sel withObject:arg];
    } else {
        () = msg![env; target performSelector:sel];
    }
}

@end

};
