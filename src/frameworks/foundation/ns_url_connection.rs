/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLConnection`.

use crate::objc::{autorelease, id, msg, nil, objc_classes, release, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLConnection: NSObject

+ (id)connectionWithRequest:(id)request // NSURLRequest *
                   delegate:(id)delegate {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithRequest:request delegate:delegate];
    autorelease(env, new)
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate {
    msg![env; this initWithRequest:request delegate:delegate startImmediately:true]
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate
     startImmediately:(bool)start_immediately {
    log!(
        "TODO: [(NSURLConnection *){:?} initWithRequest:{:?} delegate:{:?} startImmediately:{}]",
        this,
        request,
        delegate,
        start_immediately,
    );

    // Hack: immediately simulate response.
    if start_immediately && delegate != nil {
        // Check if delegate can finish loading.
        let responds_finish: bool = msg![env; delegate respondsToSelector:crate::sel!(connectionDidFinishLoading:)];
        if responds_finish {
            msg![env; delegate connectionDidFinishLoading:this];
        } else {
            // Otherwise, send a failure signal.
            let responds_fail: bool = msg![env; delegate respondsToSelector:crate::sel!(connection:didFailWithError:)];
            if responds_fail {
                msg![env; delegate connection:this didFailWithError:nil];
            }
        }
    }

    // Return 'this' instead of 'nil'.
    this
}

@end

};
