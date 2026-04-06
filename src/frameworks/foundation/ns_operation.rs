/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSOperation` and `NSOperationQueue`.

use crate::objc::{id, msg, msg_class, nil, objc_classes, SEL, ClassExports, HostObject};
use crate::frameworks::foundation::NSInteger;
use crate::frameworks::foundation::ns_thread::detach_new_thread_inner;

pub(super) struct NSOperationQueueHostObject;
impl HostObject for NSOperationQueueHostObject {}

pub(super) struct NSOperationHostObject;
impl HostObject for NSOperationHostObject {}

pub(super) struct NSInvocationOperationHostObject {
    pub target: id,
    pub sel: Option<SEL>,
    pub arg: id,
}
impl HostObject for NSInvocationOperationHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSOperationQueue: NSObject

+ (id)alloc {
    env.objc.alloc_object(this, Box::new(NSOperationQueueHostObject), &mut env.mem)
}

- (id)init { this }

- (())addOperation:(id)op {
    log_dbg!("[(NSOperationQueue*){:?} addOperation:{:?}] (executing in TRUE background thread!)", this, op);
    let sel = env.objc.lookup_selector("start").unwrap();
    // FIXED: Passed arguments in correct order and added `true` for tolerate_type_mismatch
    detach_new_thread_inner(env, sel, op, nil, true);
}

- (())setMaxConcurrentOperationCount:(NSInteger)_count { }

- (NSInteger)operationCount { 0 }

- (id)operations { 
    msg_class![env; NSArray array] 
}

- (())cancelAllOperations { }

- (())waitUntilAllOperationsAreFinished { }

@end


@implementation NSOperation: NSObject

+ (id)alloc {
    env.objc.alloc_object(this, Box::new(NSOperationHostObject), &mut env.mem)
}

- (id)init { this }

- (())start {
    () = msg![env; this main];
}

- (())main { }

- (bool)isCancelled { false }
- (bool)isFinished { true } 
- (bool)isExecuting { false }
- (bool)isReady { true }
- (bool)isConcurrent { true } 

- (())waitUntilFinished { }
- (())cancel { }

@end


@implementation NSInvocationOperation: NSOperation

+ (id)alloc {
    let host_object = Box::new(NSInvocationOperationHostObject {
        target: nil,
        sel: None,
        arg: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithTarget:(id)target selector:(SEL)sel object:(id)arg {
    let host_object = env.objc.borrow_mut::<NSInvocationOperationHostObject>(this);
    host_object.target = target;
    host_object.sel = Some(sel);
    host_object.arg = arg;
    this
}

- (())main {
    let host_object = env.objc.borrow::<NSInvocationOperationHostObject>(this);
    let target = host_object.target;
    let sel = host_object.sel.expect("NSInvocationOperation executed without a selector!");
    let arg = host_object.arg;
    
    if arg != nil {
        let _res: id = msg![env; target performSelector:sel withObject:arg];
    } else {
        let _res: id = msg![env; target performSelector:sel];
    }
}

@end

};
