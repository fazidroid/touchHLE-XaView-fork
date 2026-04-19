/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIAlertView`.

use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg, msg_super, nil, objc_classes, ClassExports};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref ALERT_DELEGATES: Mutex<HashMap<usize, id>> = Mutex::new(HashMap::new());
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIAlertView: UIView

- (id)initWithTitle:(id)title
                      message:(id)message
                     delegate:(id)delegate
            cancelButtonTitle:(id)cancelButtonTitle
            otherButtonTitles:(id)otherButtonTitles {

    log!("TODO: [(UIAlertView*){:?} initWithTitle:{:?} message:{:?} delegate:{:?} cancelButtonTitle:{:?} otherButtonTitles:{:?}]", this, title, message, delegate, cancelButtonTitle, otherButtonTitles);

    let msg_str = if message == nil { Cow::from("(nil)") } else { ns_string::to_rust_string(env, message) };
    let title_str = if title == nil { Cow::from("(nil)") } else { ns_string::to_rust_string(env, title) };
    log!("UIAlertView: title: {:?}, message: {:?}", title_str, msg_str);

    // Store delegate in global map using `this` as key
    let key = this.to_bits() as usize;
    ALERT_DELEGATES.lock().unwrap().insert(key, delegate);

    msg_super![env; this init]
}

- (())addButtonWithTitle:(id)title {
    log!("TODO: [(UIAlertView *){:?} addButtonWithTitle:{}]", this, ns_string::to_rust_string(env, title));
}

- (())show {
    log!("UIAlertView: auto-dismissing alert");
    let key = this.to_bits() as usize;
    let delegate = ALERT_DELEGATES.lock().unwrap().remove(&key).unwrap_or(nil);
    if delegate != nil {
        // Simulate tapping the first button (index 0)
        let _: () = msg![env; delegate alertView:this clickedButtonAtIndex:0];
        let _: () = msg![env; delegate alertView:this didDismissWithButtonIndex:0];
    }
    // Do not call msg_super to avoid actual display
}

@end

};
