/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stubs for SBJSON classes used by some games.

use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg_class, msg_super, objc_classes, ClassExports, HostObject};

#[derive(Default)]
struct SBJsonWriterHostObject;
impl HostObject for SBJsonWriterHostObject {}

#[derive(Default)]
struct SBJsonParserHostObject;
impl HostObject for SBJsonParserHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation SBJsonWriter: NSObject

- (id)init {
    msg_super![env; this init]
}

- (id)stringWithObject:(id)_obj {
    log_dbg!("SBJsonWriter stringWithObject: returning empty JSON object");
    ns_string::from_rust_string(env, String::from("{}"))
}

- (id)dataWithObject:(id)_obj {
    // Return empty NSData (avoids pointer issues)
    log_dbg!("SBJsonWriter dataWithObject: returning empty NSData");
    msg_class![env; NSData data]
}

@end

@implementation SBJsonParser: NSObject

- (id)init {
    msg_super![env; this init]
}

- (id)objectWithString:(id)_string {
    log_dbg!("SBJsonParser objectWithString: returning empty dictionary");
    msg_class![env; NSMutableDictionary new]
}

- (id)objectWithData:(id)_data {
    log_dbg!("SBJsonParser objectWithData: returning empty dictionary");
    msg_class![env; NSMutableDictionary new]
}

@end

};
