/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIAlertView`.

use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg, msg_super, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr};
use std::borrow::Cow;

struct UIAlertViewHostObject {
    delegate: id,
}
impl HostObject for UIAlertViewHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIAlertView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIAlertViewHostObject {
        delegate: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithTitle:(id)title
                      message:(id)message
                     delegate:(id)delegate
            cancelButtonTitle:(id)cancelButtonTitle
            otherButtonTitles:(id)otherButtonTitles {

    log!("UIAlertView init: title={:?}, msg={:?}", title, message);
    // Store the delegate passed to init in the host object
    let host = env.objc.borrow_mut::<UIAlertViewHostObject>(this);
    if delegate != nil {
        retain(env, delegate);
    }
    host.delegate = delegate;

    msg_super![env; this init]
}

- (id)delegate {
    env.objc.borrow::<UIAlertViewHostObject>(this).delegate
}

- (())setDelegate:(id)delegate {
    let host = env.objc.borrow_mut::<UIAlertViewHostObject>(this);
    let old = std::mem::replace(&mut host.delegate, delegate);
    if delegate != old {
        if delegate != nil { retain(env, delegate); }
        if old != nil { release(env, old); }
    }
}

- (())addButtonWithTitle:(id)title {
    log!("UIAlertView addButton: {}", ns_string::to_rust_string(env, title));
}

- (())show {
    log!("UIAlertView: AUTO-DISMISS (storage alert bypass)");

    // Retrieve the delegate that was set during init (or later)
    let delegate: id = msg![env; this delegate];
    if delegate != nil {
        let _: () = msg![env; delegate alertView:this clickedButtonAtIndex:0];
        let _: () = msg![env; delegate alertView:this didDismissWithButtonIndex:0];
    }

    // Do NOT call msg_super to prevent actual display.
}

@end

};
