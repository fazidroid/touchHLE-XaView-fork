// src/frameworks/ad_support.rs

use crate::dyld::HostDylib;
use crate::objc::{id, nil, objc_classes, ClassExports, HostObject};
use crate::Environment;

pub mod as_identifier_manager;

struct ASIdentifierManagerHost;
impl HostObject for ASIdentifierManagerHost {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation ASIdentifierManager: NSObject

        + (id)sharedManager {
        // 🏎️ TIP: Check your console for this log! 
        // If it appears, we know the runtime successfully found your class.
        log!("ASIdentifierManager sharedManager stub called");

        // FIX: Use 'this' directly instead of get_known_class
        env.objc.alloc_object(this, Box::new(ASIdentifierManagerHost), &mut env.mem)
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
    class_exports: &[CLASSES], 
    function_exports: &[],
    constant_exports: &[],
};
