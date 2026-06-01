/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stubs for the Burstly ad framework.

use crate::dyld::HostDylib;
use crate::objc::{
    autorelease, id, msg_class, objc_classes, ClassExports, HostObject, NSZonePtr, retain,
};
use crate::Environment;

#[derive(Default)]
struct BurstlyCurrencyProcessRequestDataHostObject;
impl HostObject for BurstlyCurrencyProcessRequestDataHostObject {}

#[derive(Default)]
struct BurstlyBannerAdViewHostObject;
impl HostObject for BurstlyBannerAdViewHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation BurstlyCurrencyProcessRequestData: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(BurstlyCurrencyProcessRequestDataHostObject), &mut env.mem)
}

+ (id)superclass {
    msg_class![env; NSObject class]
}

@end

@implementation BurstlyBannerAdView: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(BurstlyBannerAdViewHostObject), &mut env.mem)
}

// Add the class method copyWithZone:
+ (id)copyWithZone:(NSZonePtr)zone {
    log_dbg!("BurstlyBannerAdView class copyWithZone:");
    // Class objects are singletons; retain and autorelease.
    retain(env, this);
    autorelease(env, this)
}

- (id)copyWithZone:(NSZonePtr)zone {
    log_dbg!("BurstlyBannerAdView instance copyWithZone:");
    retain(env, this);
    autorelease(env, this)
}

@end

};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/PrivateFrameworks/Burstly.framework/Burstly",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};