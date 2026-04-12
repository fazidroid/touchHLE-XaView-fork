/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! `SKProduct`, `SKProductsResponse`, `SKProductsRequest`
//!
//! ROOT CAUSE FIX: `initWithProductIdentifiers:` previously returned `nil`.
//! The game then called `-start` on nil, the delegate callback
//! `productsRequest:didReceiveResponse:` never fired, and `InitializeCRMTask`
//! looped forever printing "iap: failed, will retry".
//!
//! Fix: `initWithProductIdentifiers:` now returns a real object. `-start`
//! immediately fires the delegate callback with an empty `SKProductsResponse`,
//! so the CRM task gets its answer and the boot screen advances.

use crate::objc::{autorelease, id, msg, msg_class, msg_send_no_type_checking, nil,
                  objc_classes, retain, ClassExports, HostObject};

struct SKProductsRequestHostObject {
    delegate: id,
}
impl HostObject for SKProductsRequestHostObject {}

struct SKProductsResponseHostObject;
impl HostObject for SKProductsResponseHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation SKProduct: NSObject
- (id)productIdentifier    { nil }
- (id)localizedTitle       { nil }
- (id)localizedDescription { nil }
- (id)price                { nil }
- (id)priceLocale          { nil }
@end

@implementation SKProductsResponse: NSObject
+ (id)alloc {
    env.objc.alloc_object(this, Box::new(SKProductsResponseHostObject), &mut env.mem)
}
- (id)products {
    msg_class![env; NSArray array]
}
- (id)invalidProductIdentifiers {
    msg_class![env; NSArray array]
}
@end

@implementation SKProductsRequest: NSObject

+ (id)alloc {
    let host_object = Box::new(SKProductsRequestHostObject { delegate: nil });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithProductIdentifiers:(id)_ids {
    this
}

- (())setDelegate:(id)delegate {
    retain(env, delegate);
    let host_obj = env.objc.borrow_mut::<SKProductsRequestHostObject>(this);
    let old = host_obj.delegate;
    host_obj.delegate = delegate;
    if old != nil {
        crate::objc::release(env, old);
    }
}

- (id)delegate {
    env.objc.borrow::<SKProductsRequestHostObject>(this).delegate
}

- (())start {
    let delegate = env.objc.borrow::<SKProductsRequestHostObject>(this).delegate;
    if delegate == nil {
        log!("SKProductsRequest: -start called with no delegate, skipping");
        return;
    }

    let response: id = msg_class![env; SKProductsResponse alloc];
    let response: id = msg![env; response init];
    let response = autorelease(env, response);

    // The guest binary registers all its selectors at startup via
    // register_bin_selectors(), so these will be in the selector table
    // by the time -start is called. We use if-let instead of .expect()
    // so a missing selector degrades gracefully instead of panicking.
    if let Some(sel) = env.objc.lookup_selector("productsRequest:didReceiveResponse:") {
        let _: () = msg_send_no_type_checking(env, (delegate, sel, this, response));
        log_dbg!("SKProductsRequest: fired productsRequest:didReceiveResponse:");
    } else {
        // Selector was not found — this should not happen with Asphalt 8
        // since it calls this method itself, so it will be in the binary.
        log!("SKProductsRequest: WARNING — selector productsRequest:didReceiveResponse: not found!");
    }

    if let Some(sel) = env.objc.lookup_selector("requestDidFinish:") {
        if env.objc.object_has_method_named(&env.mem, delegate, "requestDidFinish:") {
            let _: () = msg_send_no_type_checking(env, (delegate, sel, this));
        }
    }
}

- (())cancel {}

@end

};