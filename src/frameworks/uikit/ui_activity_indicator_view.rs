/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIActivityIndicatorView`.

use crate::frameworks::foundation::NSInteger;
use crate::objc::{id, msg, objc_classes, todo_objc_setter, ClassExports};

type UIActivityIndicatorViewStyle = NSInteger;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIActivityIndicatorView: UIView

- (id)initWithActivityIndicatorStyle:(UIActivityIndicatorViewStyle)_style {
    // TODO: proper init
    msg![env; this init]
}

- (())setActivityIndicatorViewStyle:(UIActivityIndicatorViewStyle)style {
    todo_objc_setter!(this, style);
}

- (())startAnimating {
    log!("TODO: [(UIActivityIndicatorView *){:?} startAnimating]", this);
}
- (())stopAnimating {
    log!("TODO: [(UIActivityIndicatorView *){:?} stopAnimating]", this);
}

- (())setHidesWhenStopped:(bool)_hides {
    // TODO
}

@end

};
