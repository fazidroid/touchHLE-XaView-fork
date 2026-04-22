/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stubs for OAuthConsumer framework.

use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr};

#[derive(Default)]
struct OAMutableURLRequestHostObject;
impl HostObject for OAMutableURLRequestHostObject {}

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
    let method = ns_string::get_static_str(env, "GET");
    method
}

- (())setHTTPMethod:(id)method {
    // stub
}

- (id)URL { nil }
- (())setURL:(id)url {}
- (id)parameters { nil }
- (())setParameters:(id)params {}
- (id)consumer { nil }
- (())setConsumer:(id)consumer {}
- (id)token { nil }
- (())setToken:(id)token {}
- (id)realm { nil }
- (())setRealm:(id)realm {}
- (id)signature { nil }
- (())setSignature:(id)signature {}
- (id)oauthParameters { nil }
- (())setOauthParameters:(id)params {}
- (bool)prepared { false }
- (())prepare {}

@end

};