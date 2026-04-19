/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIAlertView`.

use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg_super, nil, objc_classes, ClassExports};
use std::borrow::Cow;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIAlertView: UIView

- (id)initWithTitle:(id)title
                      message:(id)message
                     delegate:(id)delegate
            cancelButtonTitle:(id)cancelButtonTitle
            otherButtonTitles:(id)otherButtonTitles {

    log!("TODO: [(UIAlertView*){:?} initWithTitle:{:?} message:{:?} delegate:{:?} cancelButtonTitle:{:?} otherButtonTitles:{:?}]", this, title, message, delegate, cancelButtonTitle, otherButtonTitles);

    let msg = if message == nil { Cow::from("(nil)") } else { ns_string::to_rust_string(env, message) };
    let title = if title == nil { Cow::from("(nil)") } else { ns_string::to_rust_string(env, title) };
    log!("UIAlertView: title: {:?}, message: {:?}", title, msg);

    // Store delegate
    let host_obj = env.objc.borrow_mut::<UIAlertViewHostObject>(this);
    host_obj.delegate = delegate;
    host_obj.button_count = 0;

    msg_super![env; this init]
}

- (())addButtonWithTitle:(id)title {
    log!("TODO: [(UIAlertView *){:?} addButtonWithTitle:{}]", this, ns_string::to_rust_string(env, title));
    let host_obj = env.objc.borrow_mut::<UIAlertViewHostObject>(this);
    host_obj.button_count += 1;
}

- (())show {
    log!("UIAlertView: auto-dismissing alert");
    let host_obj = env.objc.borrow_mut::<UIAlertViewHostObject>(this);
    let delegate = host_obj.delegate;
    if delegate != nil {
        // Simulate tapping the first button (index 0)
        let clicked_sel = env.objc.get_selector("alertView:clickedButtonAtIndex:");
        if msg![env; delegate respondsToSelector:clicked_sel] {
            msg![env; delegate alertView:this clickedButtonAtIndex:0];
        }
        // Also call didDismiss if implemented
        let dismissed_sel = env.objc.get_selector("alertView:didDismissWithButtonIndex:");
        if msg![env; delegate respondsToSelector:dismissed_sel] {
            msg![env; delegate alertView:this didDismissWithButtonIndex:0];
        }
    }
}

@end

};
