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

use crate::objc::{autorelease, id, msg, msg_class, nil, objc_classes, retain, ClassExports, HostObject};

// ---------------------------------------------------------------------------
// SKProductsRequest host object — stores the delegate set before -start
// ---------------------------------------------------------------------------
struct SKProductsRequestHostObject {
    /// Strong reference to the delegate (SKProductsRequestDelegate).
    delegate: id,
}
impl HostObject for SKProductsRequestHostObject {}

// ---------------------------------------------------------------------------
// SKProductsResponse host object — wraps the (empty) result
// ---------------------------------------------------------------------------
struct SKProductsResponseHostObject;
impl HostObject for SKProductsResponseHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// ---------------------------------------------------------------------------
// SKProduct — individual purchasable item. We never populate any.
// ---------------------------------------------------------------------------
@implementation SKProduct: NSObject

- (id)productIdentifier {
    nil
}

- (id)localizedTitle {
    nil
}

- (id)localizedDescription {
    nil
}

- (id)price {
    nil
}

- (id)priceLocale {
    nil
}

@end

// ---------------------------------------------------------------------------
// SKProductsResponse — handed to the delegate. Always empty.
// ---------------------------------------------------------------------------
@implementation SKProductsResponse: NSObject

+ (id)alloc {
    let host_object = Box::new(SKProductsResponseHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)products {
    // Empty array — game iterates this to find purchasable items.
    msg_class![env; NSArray array]
}

- (id)invalidProductIdentifiers {
    msg_class![env; NSArray array]
}

@end

// ---------------------------------------------------------------------------
// SKProductsRequest — THE critical class.
// ---------------------------------------------------------------------------
@implementation SKProductsRequest: NSObject

+ (id)alloc {
    let host_object = Box::new(SKProductsRequestHostObject { delegate: nil });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithProductIdentifiers:(id)_ids { // NSSet *
    // Previously returned nil — that was the root cause of the boot freeze.
    // Return self so the caller gets a valid object.
    this
}

- (())setDelegate:(id)delegate {
    // Retain the delegate (strong ref for the lifetime of the request).
    retain(env, delegate);
    let host_obj = env.objc.borrow_mut::<SKProductsRequestHostObject>(this);
    let old = host_obj.delegate;
    host_obj.delegate = delegate;
    // Release previous delegate if any.
    if old != nil {
        crate::objc::release(env, old);
    }
}

- (id)delegate {
    env.objc.borrow::<SKProductsRequestHostObject>(this).delegate
}

- (())start {
    // Fire the delegate callback immediately with an empty response.
    // This is what unblocks InitializeCRMTask so the boot screen advances.
    let delegate = env.objc.borrow::<SKProductsRequestHostObject>(this).delegate;
    if delegate == nil {
        log!("SKProductsRequest: -start called with no delegate set, skipping callback");
        return;
    }

    // Build an empty SKProductsResponse.
    let response: id = msg_class![env; SKProductsResponse alloc];
    let response: id = msg![env; response init];
    let response = autorelease(env, response);

    // Call the mandatory delegate method:
    // -productsRequest:didReceiveResponse:
    let _: () = msg![env; delegate productsRequest:this didReceiveResponse:response];

    // Call the optional SKRequestDelegate method -requestDidFinish: if
    // the delegate implements it.
    let _: () = msg![env; delegate requestDidFinish:this];

    log_dbg!("SKProductsRequest: fired empty didReceiveResponse to delegate {:?}", delegate);
}

- (())cancel {
    // No-op — no async work is in flight.
}

@end

};
