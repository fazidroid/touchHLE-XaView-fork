//! Accounts framework stubs (ACAccountStore, ACAccountType, etc.)

use crate::dyld::HostDylib;
use crate::objc::{id, msg_class, nil, objc_classes, ClassExports, TrivialHostObject};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation ACAccountStore: NSObject

+ (id)alloc {
    let host_object = Box::new(TrivialHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    this
}

- (id)requestAccessToAccountsWithType:(id)accountType
                              options:(id)options
                            completion:(id)completion {
    log_dbg!("ACAccountStore requestAccessToAccountsWithType:... called - returning denied");
    // Return nil; real implementation would call the completion block.
    nil
}

- (id)accountsWithAccountType:(id)accountType {
    log_dbg!("ACAccountStore accountsWithAccountType: called - returning empty array");
    msg_class![env; NSArray array]
}

- (id)accountTypeWithAccountTypeIdentifier:(id)identifier {
    log_dbg!("ACAccountStore accountTypeWithAccountTypeIdentifier: called - returning dummy ACAccountType");
    let class = env.objc.get_known_class("ACAccountType", &mut env.mem);
    let obj: id = msg![env; class alloc];
    msg![env; obj init]
}

@end

@implementation ACAccountType: NSObject

- (bool)accessGranted {
    false
}

@end

};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/Accounts.framework/Accounts",
    aliases: &[],
    class_exports: &[&CLASSES],
    function_exports: &[],
    constant_exports: &[],
};