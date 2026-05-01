/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIWebView`.

use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::msg;
use crate::objc::{id, nil, objc_classes, ClassExports};
use std::borrow::Cow;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIWebView: UIView

// NSCoding implementation
- (id)initWithCoder:(id)_coder {
    todo!()
}

- (())setScalesPageToFit:(bool)_scales {
    // TODO
}
- (())setDelegate:(id)_delegate {
    // TODO
}
- (())loadRequest:(id)request {
    log!("UIWebView loadRequest: simulating immediate load finish");
    // Notify the delegate that loading finished.
    let delegate: id = msg![env; this delegate];
    if delegate != nil && env.objc.object_has_method_named(&env.mem, delegate, "webViewDidFinishLoad:") {
        let _: () = msg![env; delegate webViewDidFinishLoad:this];
    }
}

- (())loadHTMLString:(id)_string baseURL:(id)_baseURL {
    log!("TODO: [(UIWebView*) {:?} loadHTMLString:baseURL:]", this);
}

// Хак для защиты от Use-After-Free зомби-строк
- (id)stringByAppendingFormat:(id)_format {
    log!("Zombie object UIWebView called as NSString!");
    crate::objc::nil
}

@end

};
