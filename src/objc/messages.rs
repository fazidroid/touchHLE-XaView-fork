/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C messaging (`objc_msgSend` and friends).
//!
//! Resources:
//! - Apple's [Objective-C Runtime Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjCRuntimeGuide/Articles/ocrtHowMessagingWorks.html)
//! - [Apple's documentation of `objc_msgSend`](https://developer.apple.com/documentation/objectivec/1456712-objc_msgsend)
//! - Mike Ash's [objc_msgSend's New Prototype](https://www.mikeash.com/pyblog/objc_msgsends-new-prototype.html)
//! - Peter Steinberger's [Calling Super at Runtime in Swift](https://steipete.com/posts/calling-super-at-runtime/) explains `objc_msgSendSuper2`

use super::{id, nil, Class, ObjC, IMP, SEL};
use crate::abi::{CallFromHost, GuestRet};
use crate::mem::{ConstPtr, MutVoidPtr, SafeRead};
use crate::Environment;
use std::any::TypeId;

/// The core implementation of `objc_msgSend`, the main function of Objective-C.
///
/// Note that while only two parameters (usually receiver and selector) are
/// defined by the wrappers over this function, a call to an `objc_msgSend`
/// variant may have additional arguments to be forwarded (or rather, left
/// untouched) by `objc_msgSend` when it tail-calls the method implementation it
/// looks up. This is invisible to the Rust type system; we're relying on
/// [crate::abi::CallFromGuest] here.
///
/// Similarly, the return value of `objc_msgSend` is whatever value is returned
/// by the method implementation. We are relying on CallFromGuest not
/// overwriting it.
#[allow(non_snake_case)]
fn objc_msgSend_inner(
    env: &mut Environment,
    receiver: id,
    selector: SEL,
    super2: Option<Class>,
    tolerate_type_mismatch: bool,
) {
    log_dbg!(
        "Dispatching {} for {:?}",
        selector.as_str(&env.mem),
        receiver
    );
    let message_type_info = env.objc.message_type_info.take();

    // BypassNetworkError
    let sel_str = selector.as_str(&env.mem);
    // SAFE: only crash-prone selectors
            // Bypass empty/null selectors to prevent UIApplication crashes
    if sel_str.is_empty() {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if sel_str == "keyEnumerator" {
         env.cpu.regs_mut()[0] = 0;
         return;
    }

    if sel_str == "copyWithZone:" {
         env.cpu.regs_mut()[0] = receiver.to_bits();
         return;
    }

     if sel_str == "description" {
         env.cpu.regs_mut()[0] = receiver.to_bits();
         return;
    }

    if sel_str == "globallyUniqueString" {
         env.cpu.regs_mut()[0] = 0;
         return;
    }
    
    if sel_str == "sharedHTTPCookieStorage" {
        env.cpu.regs_mut()[0] = 0;
        return;
    }
    
        // Bypass UITextField isSecureTextEntry
    if sel_str == "isSecureTextEntry" {
        env.cpu.regs_mut()[0..2].fill(0); // Returns 0 (false)
        return;
    }
    
    // Bypass NSMutableDictionary keyEnumerator (safe)
    if sel_str == "keyEnumerator" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }
    // BypassNSURLQuery
    if sel_str == "query" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSCodingEncode
    if sel_str == "encodeWithCoder:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSProcessInfoUnique
    if sel_str == "globallyUniqueString" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSMutableDictionarySort
    if sel_str == "keysSortedByValueUsingSelector:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSDataDescription
    if sel_str == "description" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSRunLoopPort
    if sel_str == "addPort:forMode:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSMachPortSelector
    if sel_str == "port" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSTimeZoneDefault
    if sel_str == "defaultTimeZone" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassUIWebViewJS
    if sel_str == "stringByEvaluatingJavaScriptFromString:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSDateFormatterTimeZone
    if sel_str == "setTimeZone:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSTimeZone
    if sel_str == "knownTimeZoneNames" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    
    // BypassNSStringURLLoading
    if sel_str == "stringWithContentsOfURL:encoding:error:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassNSURLConnectionSync
    if sel_str == "sendSynchronousRequest:returningResponse:error:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if sel_str == "localizedDescription" || sel_str == "localizedFailureReason" || sel_str == "connection:didFailWithError:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if receiver == nil {
        // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjectiveC/Chapters/ocObjectsClasses.html#//apple_ref/doc/uid/TP30001163-CH11-SW7
        log_dbg!("[nil {}]", selector.as_str(&env.mem));
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassGarbagePointer
    if receiver.to_bits() >= 0xe0000000 {
        log!("WARNING: objc_msgSend received garbage pointer {:#010x}. Bypassing.", receiver.to_bits());
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let orig_class = super2.unwrap_or_else(|| ObjC::read_isa(receiver, &env.mem));
    if orig_class == nil {
        // BypassNilClassAssert
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // Traverse the chain of superclasses to find the method implementation.

    let mut class = orig_class;
    loop {
        if class == nil {
            assert!(class != orig_class);

            let class_host_object = match env.objc.get_host_object(orig_class) {
                Some(obj) => obj,
                None => {
                    log!("WARNING: objc_msgSend superclass chain lookup failed for {:?}. Bypassing.", orig_class);
                    env.cpu.regs_mut()[0..2].fill(0);
                    return;
                }
            };
            let &super::ClassHostObject {
                ref name,
                is_metaclass,
                ..
            } = class_host_object.as_any().downcast_ref().unwrap();

            // BypassMethodSelector
            if selector.as_str(&env.mem) == "methodForSelector:" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassStopLoading
            if selector.as_str(&env.mem) == "stopLoading" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassInterfaceIdiom
            if selector.as_str(&env.mem) == "userInterfaceIdiom" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassRootViewController
            if selector.as_str(&env.mem) == "setRootViewController:" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            
            // ===== NSDate copyWithZone FIX =====
            if sel_str == "copyWithZone:" {
               log!("Stub: copyWithZone (SAFE)");
               return;
             }

             if sel_str == "copy" || sel_str == "mutableCopy" {
                 log!("Stub: copy/mutableCopy (SAFE)");
                 return;
             }
             
             // ===== Redbull Update Type Confusion FIX =====
             if sel_str == "stringByReplacingOccurrencesOfString:withString:" {
                 log!("Stub: stringByReplacingOccurrencesOfString:withString: on invalid class (SAFE)");
                 env.cpu.regs_mut()[0..2].fill(0);
                 return;
             }

             // ===== Burstly superclass FIX =====
             if sel_str == "superclass" {
                 log!("Stub: superclass on invalid class (SAFE)");
                 env.cpu.regs_mut()[0..2].fill(0);
                 return;
             }

             // ===== Burstly dictionary FIX =====
             if sel_str == "dictionaryWithValuesForKeys:" {
                 log!("Stub: dictionaryWithValuesForKeys: on invalid class (SAFE)");
                 env.cpu.regs_mut()[0..2].fill(0);
                 return;
             }

             // ===== Burstly JSON Serializer FIX =====
             if sel_str == "toJSONAs:excludingInArray:withTranslations:" {
                 log!("Stub: toJSONAs:excludingInArray:withTranslations: on invalid class (SAFE)");
                 env.cpu.regs_mut()[0..2].fill(0);
                 return;
             }

             // ===== Burstly self FIX =====
             if sel_str == "self" {
                 log!("Stub: self on invalid class (SAFE)");
                 env.cpu.regs_mut()[0] = receiver.to_bits();
                 return;
             }

            panic!(
                "{} {:?} ({}class \"{}\", {:?}){} does not respond to selector \"{}\"!",
                if is_metaclass { "Class" } else { "Object" },
                receiver,
                if is_metaclass { "meta" } else { "" },
                name,
                orig_class,
                if super2.is_some() {
                    "'s superclass"
                } else {
                    ""
                },
                selector.as_str(&env.mem),
            );
        }

        let host_object = match env.objc.get_host_object(class) {
            Some(obj) => obj,
            None => {
                log!("WARNING: objc_msgSend failed to get host object for class {:?}. Bypassing.", class);
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
        };

        if let Some(&super::ClassHostObject {
            superclass,
            ref methods,
            ref name,
            ..
        }) = host_object.as_any().downcast_ref()
        {
            
            // ===== AUTO-DISMISS ALERTS SAFELY AND NUKE FROM SCREEN =====
            if name == "UIAlertView" && selector.as_str(&env.mem) == "show" {
                log!("AUTO-DISMISSING UIAlertView and nuking from screen to unfreeze game!");
                
                // Only ask for the delegate if touchHLE actually supports it!
                if env.objc.object_has_method_named(&env.mem, receiver, "delegate") {
                    let delegate: id = msg![env; receiver delegate];
                    if delegate != nil {
                        if env.objc.object_has_method_named(&env.mem, delegate, "alertView:clickedButtonAtIndex:") {
                            let zero: i32 = 0;
                            let _: () = msg![env; delegate alertView:receiver clickedButtonAtIndex:zero];
                        }
                        if env.objc.object_has_method_named(&env.mem, delegate, "alertView:didDismissWithButtonIndex:") {
                            let zero: i32 = 0;
                            let _: () = msg![env; delegate alertView:receiver didDismissWithButtonIndex:zero];
                        }
                    }
                }
                
                // Drop the invisible shield!
                if env.objc.object_has_method_named(&env.mem, receiver, "setHidden:") {
                    let _: () = msg![env; receiver setHidden:true];
                }
                if env.objc.object_has_method_named(&env.mem, receiver, "setUserInteractionEnabled:") {
                    let _: () = msg![env; receiver setUserInteractionEnabled:false];
                }
                if env.objc.object_has_method_named(&env.mem, receiver, "removeFromSuperview") {
                    let _: () = msg![env; receiver removeFromSuperview];
                }
                
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassUIPasteboard
            if name == "UIPasteboard" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }


            // BypassNSOperationQueue
            if name == "NSOperationQueue" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassNSInvocationOperation
            if name == "NSInvocationOperation" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassNSOperationQueue (early catch)
            if name == "NSOperationQueue" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // Skip method lookup on first iteration if this is the super-call
            // variant of objc_msgSend (look up the superclass first)
            if super2.is_some() && class == orig_class {
                class = superclass;
                continue;
            }

            if let Some(imp) = methods.get(&selector) {
                log_dbg!("Found method on: {}", name);
                match imp {
                    IMP::Host(host_imp) => {
                        // TODO: do type checks when calling GuestIMPs too.
                        // That requires using Objective-C type strings,
                        // rather than Rust types, and should probably
                        // warn rather than panicking,
                        // because apps might rely on type punning.
                        if let Some((sent_type_id, sent_type_desc)) = message_type_info {
                            let (expected_type_id, expected_type_desc) = host_imp.type_info();
                            if sent_type_id != expected_type_id {
                                let msg = format!(
                                    "\
Type mismatch when sending message {} to {:?}!
- Message has type: {:?} / {}
- Method expects type: {:?} / {}",
                                    selector.as_str(&env.mem),
                                    receiver,
                                    sent_type_id,
                                    sent_type_desc,
                                    expected_type_id,
                                    expected_type_desc
                                );
                                if tolerate_type_mismatch {
                                    log!("Warning: {}", msg);
                                } else {
                                    panic!("{}", msg);
                                }
                            }
                        }
                        host_imp.call_from_guest(env)
                    }
                    // We can't create a new stack frame, because that would
                    // interfere with pass-through of stack arguments.
                    IMP::Guest(guest_imp) => guest_imp.call_without_pushing_stack_frame(env),
                }
                return;
            } else {
                class = superclass;
            }
        } else if let Some(&super::UnimplementedClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {

            // BypassUIPasteboard
            if name == "UIPasteboard" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }


            // BypassNSOperationQueue
            if name == "NSOperationQueue" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassNSInvocationOperation
            if name == "NSInvocationOperation" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassNSOperationQueue (early catch)
            if name == "NSOperationQueue" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassGKSession

            // BypassAVAudioSession
            if name == "AVAudioSession" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            if name == "GKSession" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // FakeAccessoryManager
            if name == "EAAccessoryManager" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassMailCompose
            if name == "MFMailComposeViewController" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassMessageCompose
            if name == "MFMessageComposeViewController" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // FakeAdManager
            if name == "ASIdentifierManager" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            panic!(
                "Class \"{}\" ({:?}) is unimplemented. Call to {} method \"{}\".",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem),
            );
        } else if let Some(&super::FakeClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {

            // BypassUIPasteboard
            if name == "UIPasteboard" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }


            // BypassNSOperationQueue
            if name == "NSOperationQueue" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassNSInvocationOperation
            if name == "NSInvocationOperation" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            // BypassNSOperationQueue (early catch)
            if name == "NSOperationQueue" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            log!(
                "Call to faked class \"{}\" ({:?}) {} method \"{}\". Behaving as if message was sent to nil.",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem),
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        } else {
            panic!(
                "Item {class:?} in superclass chain of object {receiver:?}'s class {orig_class:?} has an unexpected host object type."
            );
        }
    }
}

/// Standard variant of `objc_msgSend`. See [objc_msgSend_inner].
#[allow(non_snake_case)]
pub(super) fn objc_msgSend(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
    )
}

#[allow(non_snake_case)]
pub(crate) fn _touchHLE_objc_msgSend_tolerant(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ true,
    )
}

/// Variant of `objc_msgSend` for methods that return a struct via a pointer.
/// See [objc_msgSend_inner].
///
/// The first parameter here is the pointer for the struct return. This is an
/// ABI detail that is usually hidden and handled behind-the-scenes by
/// [crate::abi], but `objc_msgSend` is a special case because of the
/// pass-through behaviour. Of course, the pass-through only works if the [IMP]
/// also has the pointer parameter. The caller therefore has to pick the
/// appropriate `objc_msgSend` variant depending on the method it wants to call.
pub(super) fn objc_msgSend_stret(
    env: &mut Environment,
    _stret: MutVoidPtr,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
    )
}

#[allow(non_snake_case)]
pub(crate) fn _touchHLE_objc_msgSend_stret_tolerant(
    env: &mut Environment,
    _stret: MutVoidPtr,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ true,
    )
}

#[repr(C, packed)]
/// A pointer to this struct replaces the normal receiver parameter for
/// `objc_msgSendSuper2` and [msg_send_super2].
pub struct objc_super {
    pub receiver: id,
    /// If this is used with `objc_msgSendSuper` (not implemented here, TODO),
    /// this is a pointer to the superclass to look up the method on.
    /// If this is used with `objc_msgSendSuper2`, this is a pointer to a class
    /// and the superclass will be looked up from it.
    pub class: Class,
}
unsafe impl SafeRead for objc_super {}

/// Variant of `objc_msgSend` for supercalls. See [objc_msgSend_inner].
///
/// This variant has a weird ABI because it needs to receive an additional piece
/// of information (a class pointer), but it can't actually take this as an
/// extra parameter, because that would take one of the argument slots reserved
/// for arguments passed onto the method implementation. Hence the [objc_super]
/// pointer in place of the normal [id].
#[allow(non_snake_case)]
pub(super) fn objc_msgSendSuper2(
    env: &mut Environment,
    super_ptr: ConstPtr<objc_super>,
    selector: SEL,
) {
    let objc_super { receiver, class } = env.mem.read(super_ptr);

    // Rewrite first argument to match the normal ABI.
    crate::abi::write_next_arg(&mut 0, env.cpu.regs_mut(), &mut env.mem, receiver);

    objc_msgSend_inner(
        env,
        receiver,
        selector,
        /* super2: */ Some(class),
        /* tolerate_type_mismatch: */ false,
    )
}

/// Trait that assists with type-checking of [msg_send]'s arguments.
///
/// - Statically constrains the types of [msg_send]'s arguments so that the
///   first two are always [id] and [SEL].
/// - Provides the type ID to enable dynamic type checking of subsequent
///   arguments and the return type.
///
/// See `impl_HostIMP` for implementations. See also [MsgSendSuperSignature].
pub trait MsgSendSignature: 'static {
    /// Get the [TypeId] and a human-readable description for this signature.
    fn type_info() -> (TypeId, &'static str) {
        #[cfg(debug_assertions)]
        let type_name = std::any::type_name::<Self>();
        // Avoid wasting space on type names in release builds. At the time of
        // writing this saves about 36KB.
        #[cfg(not(debug_assertions))]
        let type_name = "[description unavailable in release builds]";
        (TypeId::of::<Self>(), type_name)
    }
}

/// Wrapper around [objc_msgSend] which, together with [msg], makes it easy to
/// send messages in host code. Warning: all types are inferred from the
/// call-site and they may not be checked, so be very sure you get them correct!
pub fn msg_send<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSignature>::type_info());
    if R::SIZE_IN_MEM.is_some() {
        (objc_msgSend_stret as fn(&mut Environment, MutVoidPtr, id, SEL)).call_from_host(env, args)
    } else {
        (objc_msgSend as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

pub fn msg_send_no_type_checking<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    if R::SIZE_IN_MEM.is_some() {
        (_touchHLE_objc_msgSend_stret_tolerant as fn(&mut Environment, MutVoidPtr, id, SEL))
            .call_from_host(env, args)
    } else {
        (_touchHLE_objc_msgSend_tolerant as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

/// Counterpart of [MsgSendSignature] for [msg_send_super2].
pub trait MsgSendSuperSignature: 'static {
    /// Signature with the [objc_super] pointer replaced by [id].
    type WithoutSuper: MsgSendSignature;
}

/// [msg_send] but for super-calls (calls [objc_msgSendSuper2]). You probably
/// want to use [msg_super] rather than calling this directly.
pub fn msg_send_super2<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSuperSignature,
    R: GuestRet,
{
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSuperSignature>::WithoutSuper::type_info());
    if R::SIZE_IN_MEM.is_some() {
        todo!() // no stret yet
    } else {
        (objc_msgSendSuper2 as fn(&mut Environment, ConstPtr<objc_super>, SEL))
            .call_from_host(env, args)
    }
}

/// Macro for sending a message which imitates the Objective-C messaging syntax.
/// See [msg_send] for the underlying implementation. Warning: all types are
/// inferred from the call-site and they may not be checked, so be very sure you
/// get them correct!
///
/// ```ignore
/// msg![env; foo setBar:bar withQux:qux];
/// ```
///
/// desugars to:
///
/// ```ignore
/// {
///     let sel = env.objc.lookup_selector("setFoo:withBar").unwrap();
///     msg_send(env, (foo, sel, bar, qux))
/// }
/// ```
///
/// Note that argument values that aren't a bare single identifier like `foo`
/// need to be bracketed.
///
/// See also [msg_class], if you want to send a message to a class.
#[macro_export]
macro_rules! msg {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel)
                .expect("Unknown selector");
            let args = ($receiver, sel, $($arg1, $($argn),*)?);
            $crate::objc::msg_send($env, args)
        }
    }
}
pub use crate::msg; // #[macro_export] is weird...

/// Variant of [msg] for super-calls.
///
/// Unlike the other variants, this macro can only be used within
/// [crate::objc::objc_classes], because it relies on that macro defining a
/// constant containing the name of the current class.
///
/// ```ignore
/// msg_super![env; this init]
/// ```
///
/// desugars to something like this, if the current class is `SomeClass`:
///
/// ```ignore
/// {
///     let super_arg_ptr = push_to_stack(env, objc_super {
///         receiver: this,
///         class: env.objc.get_known_class("SomeClass", &mut env.mem),
///     });
///     let sel = env.objc.lookup_selector("init").unwrap();
///     let res = msg_send_super2(env, (super_arg_ptr, sel));
///     pop_from_stack::<objc_super>(env);
///     res
/// }
/// ```
#[macro_export]
macro_rules! msg_super {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let class = $env.objc.get_known_class(
                _OBJC_CURRENT_CLASS,
                &mut $env.mem
            );
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel)
                .expect("Unknown selector");

            let sp = &mut $env.cpu.regs_mut()[$crate::cpu::Cpu::SP];
            let old_sp = *sp;
            *sp -= $crate::mem::guest_size_of::<$crate::objc::objc_super>();
            let super_ptr = $crate::mem::Ptr::from_bits(*sp);
            $env.mem.write(super_ptr, $crate::objc::objc_super {
                receiver: $receiver,
                class,
            });

            let args = (super_ptr.cast_const(), sel, $($arg1, $($argn),*)?);
            let res = $crate::objc::msg_send_super2($env, args);

            $env.cpu.regs_mut()[$crate::cpu::Cpu::SP] = old_sp;

            res
        }
    }
}
pub use crate::msg_super; // #[macro_export] is weird...

/// Variant of [msg] for sending a message to a named class. Useful for calling
/// class methods, especially `new`.
///
/// ```ignore
/// msg_class![env; SomeClass alloc]
/// ```
///
/// desugars to:
///
/// ```ignore
/// msg![env; (env.objc.get_known_class("SomeClass", &mut env.mem)) alloc]
/// ```
#[macro_export]
macro_rules! msg_class {
    [$env:expr; $receiver_class:ident $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let class = $env.objc.get_known_class(
                stringify!($receiver_class),
                &mut $env.mem
            );
            $crate::objc::msg![$env; class $name $(: $arg1 $($($namen)?: $argn)*)?]
        }
    }
}
pub use crate::msg_class; // #[macro_export] is weird...

/// Shorthand for `let _: id = msg![env; object retain];`
pub fn retain(env: &mut Environment, object: id) -> id {
    if object == nil {
        // fast path
        return nil;
    }
    msg![env; object retain]
}

/// Shorthand for `() = msg![env; object release];`
pub fn release(env: &mut Environment, object: id) {
    if object == nil {
        // fast path
        return;
    }
    msg![env; object release]
}

/// Shorthand for `let _: id = msg![env; object autorelease];`
pub fn autorelease(env: &mut Environment, object: id) -> id {
    if object == nil {
        // fast path
        return nil;
    }
    msg![env; object autorelease]
}
