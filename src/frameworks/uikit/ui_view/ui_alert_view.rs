/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIAlertView`.

use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg, msg_super, nil, objc_classes, ClassExports};
use std::borrow::Cow;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIAlertView: UIView

- (id)initWithTitle:(id)title
                      message:(id)message
                     delegate:(id)delegate
            cancelButtonTitle:(id)cancelButtonTitle
            otherButtonTitles:(id)otherButtonTitles {

    log!("UIAlertView init: title={:?}, msg={:?}", title, message);
    msg_super![env; this init]
}

- (())addButtonWithTitle:(id)title {
    log!("UIAlertView addButton: {}", ns_string::to_rust_string(env, title));
}

- (())show {
    log!("UIAlertView: AUTO-DISMISS (storage alert bypass)");

    // Retrieve the delegate that was set during init
    let delegate: id = msg![env; this delegate];
    if delegate != nil {
        let _: () = msg![env; delegate alertView:this clickedButtonAtIndex:0];
        let _: () = msg![env; delegate alertView:this didDismissWithButtonIndex:0];
    }

    // Do NOT call msg_super to prevent actual display.
}

@end

};
