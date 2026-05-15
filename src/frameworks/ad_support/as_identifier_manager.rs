use crate::frameworks::foundation::ns_string::from_rust_string;
use crate::objc::{autorelease, id, nil, objc_classes, retain, ClassExports, HostObject, msg, msg_class};
use std::cell::Cell;

thread_local! {
    static SHARED_MANAGER: Cell<id> = Cell::new(nil);
}

// ── NSUUID stub ──────────────────────────────────────────────────────────────
// touchHLE doesn't implement NSUUID yet, but ASIdentifierManager must return
// one from -advertisingIdentifier. This minimal stub handles the selectors
// that Real Racing 3 (and similar games) call on the returned object.

pub struct NSUUIDHostObject {
    /// The UUID string stored as a retained NSString.
    pub uuid_string: id,
}
impl HostObject for NSUUIDHostObject {}

// ── ASIdentifierManager ──────────────────────────────────────────────────────

pub struct ASIdentifierManagerHostObject {
    pub advertising_identifier: id,
}
impl HostObject for ASIdentifierManagerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
    (env, this, _cmd);

    // ── NSUUID ───────────────────────────────────────────────────────────────

    @implementation NSUUID: NSObject

    // +alloc is inherited from NSObject; -init initialises with a zero UUID.
    - (id)init {
        let uuid_str = from_rust_string(env, "00000000-0000-0000-0000-000000000000".to_string());
        retain(env, uuid_str);
        env.objc.replace_host_object(
            this,
            Box::new(NSUUIDHostObject { uuid_string: uuid_str }),
        );
        this
    }

    // Designated initialiser: store the caller-supplied UUID string.
    - (id)initWithUUIDString:(id)string {
        retain(env, string);
        env.objc.replace_host_object(
            this,
            Box::new(NSUUIDHostObject { uuid_string: string }),
        );
        this
    }

    // Primary accessor — returns an autoreleased NSString.
    - (id)UUIDString {
        let s = env.objc.borrow::<NSUUIDHostObject>(this).uuid_string;
        autorelease(env, s);
        s
    }

    @end

    // ── ASIdentifierManager ──────────────────────────────────────────────────

    @implementation ASIdentifierManager: NSObject

    // Returns the shared singleton instance.
    + (id)sharedManager {
        let existing = SHARED_MANAGER.with(|c| c.get());
        if existing != nil {
            return existing;
        }
        let instance: id = msg_class![env; ASIdentifierManager alloc];
        let instance: id = msg![env; instance init];
        SHARED_MANAGER.with(|c| c.set(instance));
        instance
    }

    - (id)init {
        // `alloc` stores a TrivialHostObject; we cannot borrow_mut to the real
        // type until replace_host_object swaps the box — same pattern as NSBundle.

        // Build a fake but valid zeroed IDFA so callers never receive nil.
        // NSUUID is now defined above so msg_class! will succeed.
        let uuid_str = from_rust_string(env, "00000000-0000-0000-0000-000000000000".to_string());
        let uuid: id = msg_class![env; NSUUID alloc];
        let uuid: id = msg![env; uuid initWithUUIDString:uuid_str];
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

    // Returns the advertising identifier (IDFA) as an NSUUID.
    - (id)advertisingIdentifier {
        let uuid = env
            .objc
            .borrow::<ASIdentifierManagerHostObject>(this)
            .advertising_identifier;
        // Return an autoreleased reference — callers don't own this object.
        autorelease(env, uuid);
        uuid
    }

    - (bool)isAdvertisingTrackingEnabled {
        false
    }

    @end
};
