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
use std::sync::{Mutex, OnceLock};

static ALERT_DELEGATES: OnceLock<Mutex<HashMap<usize, id>>> = OnceLock::new();

fn delegates() -> &'static Mutex<HashMap<usize, id>> {
    ALERT_DELEGATES.get_or_init(|| Mutex::new(HashMap::new()))
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

    // Store delegate globally, keyed by alert instance
    let key = this.to_bits() as usize;
    delegates().lock().unwrap().insert(key, delegate);

    msg_super![env; this init]
}

- (())addButtonWithTitle:(id)title {
    log!("TODO: [(UIAlertView *){:?} addButtonWithTitle:{}]", this, ns_string::to_rust_string(env, title));
}

- (())show {
    log!("UIAlertView: auto-dismissing alert");
    let key = this.to_bits() as usize;
    let delegate = delegates().lock().unwrap().remove(&key).unwrap_or(nil);
    if delegate != nil {
        let _: () = msg![env; delegate alertView:this clickedButtonAtIndex:0];
        let _: () = msg![env; delegate alertView:this didDismissWithButtonIndex:0];
    }
    // Suppress actual UI display
}

@end

};
