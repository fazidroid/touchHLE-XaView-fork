/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stubs for OAuthConsumer framework classes.

use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr};

#[derive(Default)]
struct OAMutableURLRequestHostObject;
impl HostObject for OAMutableURLRequestHostObject {}

#[derive(Default)]
struct OAServiceTicketHostObject;
impl HostObject for OAServiceTicketHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation OAMutableURLRequest: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(OAMutableURLRequestHostObject), &mut env.mem)
}

- (id)initWithURL:(id)url consumer:(id)consumer token:(id)token realm:(id)realm {
    msg![env; this init]
}

- (id)HTTPMethod {
    // Return a default HTTP method (e.g., GET)
    let method = ns_string::get_static_str(env, "GET");
    method
}

- (())setHTTPMethod:(id)method {
    // Stub - ignore
}

- (id)URL {
    return nil;
}

- (())setURL:(id)url {
    // Stub
}

- (id)parameters {
    return nil;
}

- (())setParameters:(id)parameters {
    // Stub
}

- (id)consumer {
    return nil;
}

- (())setConsumer:(id)consumer {
    // Stub
}

- (id)token {
    return nil;
}

- (())setToken:(id)token {
    // Stub
}

- (id)realm {
    return nil;
}

- (())setRealm:(id)realm {
    // Stub
}

- (id)signature {
    return nil;
}

- (())setSignature:(id)signature {
    // Stub
}

- (id)oauthParameters {
    return nil;
}

- (())setOauthParameters:(id)oauthParameters {
    // Stub
}

- (bool)prepared {
    return false;
}

- (())prepare {
    // Stub
}

- (id)OAuthMutableURLRequest {
    return this;
}

@end

@implementation OAServiceTicket: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(OAServiceTicketHostObject), &mut env.mem)
}

- (id)initWithRequest:(id)request response:(id)response data:(id)data didSucceed:(bool)didSucceed {
    msg![env; this init]
}

- (id)request {
    return nil;
}

- (id)response {
    return nil;
}

- (id)data {
    return nil;
}

- (bool)didSucceed {
    return true;
}

@end

};
