use crate::frameworks::foundation::ns_string::from_rust_string;
use crate::objc::{autorelease, id, nil, objc_classes, retain, ClassExports, HostObject, msg, msg_class};
use std::cell::Cell;

thread_local! {
    static SHARED_MANAGER: Cell<id> = Cell::new(nil);
}

// ── NSUUID stub ──────────────────────────────────────────────────────────────
pub struct NSUUIDHostObject {
    pub uuid_string: id,
}
impl HostObject for NSUUIDHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation NSUUID: NSObject

    // +alloc is inherited; -init is the default (no-op).
    - (id)init {
        let uuid_str = from_rust_string(env, "00000000-0000-0000-0000-000000000000".to_string());
        retain(env, uuid_str);
        env.objc.replace_host_object(
            this,
            Box::new(NSUUIDHostObject { uuid_string: uuid_str }),
        );
        this
    }

    - (id)UUIDString {
        let s = env.objc.borrow::<NSUUIDHostObject>(this).uuid_string;
        autorelease(env, s);
        s
    }

    @end

    // ── ASIdentifierManager ──────────────────────────────────────────────────

    @implementation ASIdentifierManager: NSObject

    + (id)sharedManager {
        let existing = SHARED_MANAGER.with(|c| c.get());
        if existing != nil {
            return existing;
        }
        let instance: id = msg_class![env; ASIdentifierManager alloc];
        let instance: id = msg![env; instance init];
        SHARED_MANAGER.with(|c| c.set(instance));
        autorelease(env, instance)
    }

    - (id)init {
        // Create an NSUUID instance without calling any initialiser.
        let uuid_class = env.objc.get_known_class("NSUUID", &mut env.mem);
        let uuid: id = msg![env; uuid_class alloc];
        let uuid_str = from_rust_string(env, "00000000-0000-0000-0000-000000000000".to_string());
        retain(env, uuid_str);
        env.objc.replace_host_object(uuid, Box::new(NSUUIDHostObject { uuid_string: uuid_str }));
        // Keep the UUID alive for the lifetime of this singleton.
        retain(env, uuid);

        env.objc.replace_host_object(
            this,
            Box::new(ASIdentifierManagerHostObject {
                advertising_identifier: uuid,
            }),
        );
        this
    }

    - (id)advertisingIdentifier {
        let uuid = env
            .objc
            .borrow::<ASIdentifierManagerHostObject>(this)
            .advertising_identifier;
        autorelease(env, uuid);
        uuid
    }

    - (bool)isAdvertisingTrackingEnabled {
        false
    }

    @end
};