// src/frameworks/ad_support.rs

use crate::dyld::HostDylib;
use crate::objc::{id, nil, objc_classes, ClassExports, HostObject};
use crate::Environment;

struct ASIdentifierManagerHost;
impl HostObject for ASIdentifierManagerHost {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation ASIdentifierManager: NSObject

    + (id)sharedManager {
        log!("ASIdentifierManager sharedManager stub called");
        let class = env.objc.get_known_class("ASIdentifierManager", &mut env.mem);
        env.objc.alloc_object(class, Box::new(ASIdentifierManagerHost), &mut env.mem)
    }

    - (id)advertisingIdentifier {
        // Returning nil is perfectly safe; legacy games have fallbacks for this
        nil
    }

    - (bool)isAdvertisingTrackingEnabled {
        false
    }

    @end
};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/AdSupport.framework/AdSupport",
    aliases: &[],
    class_exports: CLASSES,
    function_exports: &[],
    constant_exports: &[],
};
