/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIAlertView`.

use crate::frameworks::foundation::ns_string;
use crate::frameworks::uikit::ui_view::UIViewHostObject;
use crate::objc::{id, impl_HostObject_with_superclass, msg, msg_super, nil, objc_classes, release, retain, ClassExports, NSZonePtr};

struct UIAlertViewHostObject {
    superclass: UIViewHostObject,
    delegate: id,
}
impl_HostObject_with_superclass!(UIAlertViewHostObject);

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIAlertView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIAlertViewHostObject {
        superclass: Default::default(),
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
    if delegate != nil {
        retain(env, delegate);
    }
    let host = env.objc.borrow_mut::<UIAlertViewHostObject>(this);
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
    // The delegate will be called by the game, not by us.
    // Our PlatformAlertViewDelegate now implements the method.
}

@end

@implementation PlatformAlertViewDelegate: NSObject

- (())alertView:(id)alertView clickedButtonAtIndex:(i32)buttonIndex {
    log!("PlatformAlertViewDelegate - clicked button {}", buttonIndex);
    // In a real app, you'd handle the button action.
    // For the emulator, just log.
}

@end

};
