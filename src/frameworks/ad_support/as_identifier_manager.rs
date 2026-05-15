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

// ── ASIdentifierManagerHostObject ───────────────────────────────────────────
pub struct ASIdentifierManagerHostObject {
    pub advertising_identifier: id,
}
impl HostObject for ASIdentifierManagerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    @implementation NSUUID: NSObject
    
    - (())getUUIDBytes:(crate::mem::MutVoidPtr)uuid_bytes {
    if !uuid_bytes.is_null() {
        let slice = env.mem.bytes_at_mut(uuid_bytes.cast::<u8>(), 16);
        for b in slice.iter_mut() {
            *b = 0;
        }
    }
}

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
        let uuid_class = env.objc.get_known_class("NSUUID", &mut env.mem);
        let uuid: id = msg![env; uuid_class alloc];
        let uuid_str = from_rust_string(env, "00000000-0000-0000-0000-000000000000".to_string());
        retain(env, uuid_str);
        env.objc.replace_host_object(uuid, Box::new(NSUUIDHostObject { uuid_string: uuid_str }));
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