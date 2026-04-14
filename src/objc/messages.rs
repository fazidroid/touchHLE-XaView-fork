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

// StoreRootViewControllers
static ROOT_VC_STORE: std::sync::Mutex<Option<std::collections::HashMap<u32, u32>>> =
    std::sync::Mutex::new(None);

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

    // TraceAudioCalls
    let sel_name = selector.as_str(&env.mem);
    if sel_name.contains("udio") || sel_name.contains("ound") || sel_name.contains("olume") {
        println!("AUDIO_TRACE: [{:?} {}]", receiver, sel_name);
    } 

    let message_type_info = env.objc.message_type_info.take();
    let message_type_info = env.objc.message_type_info.take();

    let sel_str = selector.as_str(&env.mem);

    // ===== URL Tracker & Telemetry Bypasses =====
    if sel_str == "HTTPMethod" || sel_str == "host" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }
    
    if sel_str == "addValue:forHTTPHeaderField:" || sel_str == "setValue:forHTTPHeaderField:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if sel_str == "sortedArrayUsingSelector:" {
        env.cpu.regs_mut()[0] = receiver.to_bits();
        return;
    }
    // ============================================

    // SAFE: only crash-prone selectors
    if sel_str.is_empty() {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if sel_str == "keyEnumerator" || sel_str == "globallyUniqueString" ||
       sel_str == "sharedHTTPCookieStorage" || sel_str == "isSecureTextEntry" || sel_str == "query" || sel_str == "encodeWithCoder:" || sel_str == "keysSortedByValueUsingSelector:" ||
       sel_str == "description" || sel_str == "addPort:forMode:" || sel_str == "port" || sel_str == "defaultTimeZone" || sel_str == "stringByEvaluatingJavaScriptFromString:" ||
       sel_str == "setTimeZone:" || sel_str == "stringWithContentsOfURL:encoding:error:" || sel_str == "sendSynchronousRequest:returningResponse:error:" || sel_str == "localizedDescription" ||
       sel_str == "localizedFailureReason" || sel_str == "connection:didFailWithError:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if sel_str == "copyWithZone:" {
         env.cpu.regs_mut()[0] = receiver.to_bits();
         return;
    }

    if receiver == nil {
        log_dbg!("[nil {}]", selector.as_str(&env.mem));
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    // BypassGarbagePointer
    if receiver.to_bits() >= 0xe0000000 {
        log!(
            "WARNING: objc_msgSend received garbage pointer {:#010x}. Bypassing.",
            receiver.to_bits()
        );
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let orig_class = super2.unwrap_or_else(|| ObjC::read_isa(receiver, &env.mem));
    if orig_class == nil {
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
                    log!(
                        "WARNING: objc_msgSend superclass chain lookup failed for {:?}. Bypassing.",
                        orig_class
                    );
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
            // SafeRootViewControllerHook
            if selector.as_str(&env.mem) == "setRootViewController:" {
                let vc: id = crate::mem::Ptr::from_bits(env.cpu.regs()[2]);
                echo!(
                    "SafeHook: setRootViewController: Window: {:?}, VC: {:?}",
                    receiver,
                    vc
                );

                if vc != nil {
                    let mut store_lock = ROOT_VC_STORE.lock().unwrap();
                    if store_lock.is_none() {
                        *store_lock = Some(std::collections::HashMap::new());
                    }
                    store_lock
                        .as_mut()
                        .unwrap()
                        .insert(receiver.to_bits(), vc.to_bits());
                    drop(store_lock);

                    // SaveCpuState
                    let saved_regs = env.cpu.regs().to_vec();
                    let view: id = crate::msg![env; vc view];
                    if view != nil {
                        let sel_add = env.objc.lookup_selector("addSubview:").unwrap();
                        let _: () =
                            crate::objc::msg_send_no_type_checking(env, (receiver, sel_add, view));

                        let sel_key = env.objc.lookup_selector("makeKeyAndVisible").unwrap();
                        let _: () =
                            crate::objc::msg_send_no_type_checking(env, (receiver, sel_key));

                        *crate::libc::stdlib::HACK_MAIN_WINDOW.lock().unwrap() = receiver.to_bits();
                    }

                    // RestoreCpuState (Crucial for AppPicker stability)
                    env.cpu.regs_mut().copy_from_slice(&saved_regs);
                }

                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // FakeRootViewGetter
            if selector.as_str(&env.mem) == "rootViewController" {
                let mut vc_bits = 0;
                if let Some(store) = ROOT_VC_STORE.lock().unwrap().as_ref() {
                    vc_bits = store.get(&receiver.to_bits()).copied().unwrap_or(0);
                }
                echo!(
                    "WARNING: Hooked rootViewController! Returning {:#x}",
                    vc_bits
                );
                env.cpu.regs_mut()[0] = vc_bits;
                env.cpu.regs_mut()[1] = 0;
                return;
            }
            // BypassTimeZone
            if selector.as_str(&env.mem) == "defaultTimeZone" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassWebViewJS
            if selector.as_str(&env.mem) == "stringByEvaluatingJavaScriptFromString:" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }

            panic!(
                "{} {:?} ({}class \"{}\", {:?}){} does not respond to selector \"{}\"!",
            // ===== THE NUCLEAR OPTION: GLOBAL PANIC BYPASS =====
            // Instead of crashing the emulator when a method is missing, we safely print a warning and return 0 (nil).
            // This instantly bypasses EVERY missing ad network and tracking method Gameloft throws at us!
            log!(
                "SAFE BYPASS: {} {:?} ({}class \"{}\", {:?}){} does not respond to selector \"{}\"! Returning 0 to prevent crash.",
                if is_metaclass { "Class" } else { "Object" },
                receiver,
                if is_metaclass { "meta" } else { "" },
                name,
                orig_class,
                if super2.is_some() { "'s superclass" } else { "" },
                selector.as_str(&env.mem),
            );
            
            if sel_str == "self" {
                env.cpu.regs_mut()[0] = receiver.to_bits(); // Return self
            } else {
                env.cpu.regs_mut()[0..2].fill(0); // Return nil/0
            }
            return;
        }

        // =========================================================================
        // PRE-CHECK BYPASSES: Evaluated in a tight scope to prevent borrow conflicts
        // =========================================================================
        let mut do_video_bypass = false;
        let mut do_alert_bypass = false;

        { // The borrow of `env.objc` starts here and drops safely at the end of the block
            if let Some(host_object) = env.objc.get_host_object(class) {
                if let Some(&super::ClassHostObject { ref name, .. }) = host_object.as_any().downcast_ref() {
                    if name == "MPMoviePlayerController" || name == "MPMoviePlayerViewController" {
                        if sel_str == "play" || sel_str == "stop" {
                            do_video_bypass = true;
                        }
                    }
                    if name == "UIAlertView" && sel_str == "show" {
                        do_alert_bypass = true;
                    }
                }
            }
        } 

        // =========================================================================
        // EXECUTE BYPASSES: Safe to use `msg!` because `env.objc` is no longer borrowed
        // =========================================================================
        if do_video_bypass {
            println!("GAMELOFT BYPASS: Auto-finishing movie player to prevent infinite hang!");
            let center_class = env.objc.get_known_class("NSNotificationCenter", &mut env.mem);
            if center_class != nil {
                let center: id = msg![env; center_class defaultCenter];
                let notif1 = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackStateDidChangeNotification".to_string());
                let _: () = msg![env; center postNotificationName:notif1 object:receiver];
                let notif2 = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackDidFinishNotification".to_string());
                let _: () = msg![env; center postNotificationName:notif2 object:receiver];
            }
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }

        if do_alert_bypass {
            println!("AUTO-DISMISSING UIAlertView and nuking from screen to unfreeze game!");
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

        // =========================================================================
        // NORMAL METHOD EXECUTION
        // =========================================================================
        let host_object = match env.objc.get_host_object(class) {
            Some(obj) => obj,
            None => {
                log!(
                    "WARNING: objc_msgSend failed to get host object for class {:?}. Bypassing.",
                    class
                );
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
                        if let Some((sent_type_id, sent_type_desc)) = message_type_info {
                            let (expected_type_id, expected_type_desc) = host_imp.type_info();
                            if sent_type_id != expected_type_id {
                                let msg = format!(
                                    "Type mismatch when sending message {} to {:?}!\n- Message has type: {:?} / {}\n- Method expects type: {:?} / {}",
                                    selector.as_str(&env.mem), receiver, sent_type_id, sent_type_desc, expected_type_id, expected_type_desc
                                );
                                if tolerate_type_mismatch {
                                    println!("Warning: {}", msg);
                                } else {
                                    panic!("{}", msg);
                                }
                            }
                        }
                        host_imp.call_from_guest(env)
                    }
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
            // BypassGKSession
            // SKPaymentQueue and SKProductsRequest now have real host implementations — do NOT bypass them here.
            if name == "GKSession" || name == "GKLocalPlayer" || name == "Reachability" {
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
            // BypassTextTokenizer
            if name == "UITextInputStringTokenizer" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassBarButtonItem
            if name == "UIBarButtonItem" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            // BypassGCController
            if name == "GCController" {
                env.cpu.regs_mut()[0..2].fill(0);
                return;
            }
            panic!(
                "Class \"{}\" ({:?}) is unimplemented. Call to {} method \"{}\".",
            // ===== THE NUCLEAR OPTION: GLOBAL CLASS PANIC BYPASS =====
            log!(
                "SAFE BYPASS: Class \"{}\" ({:?}) is unimplemented. Call to {} method \"{}\". Returning 0 to prevent crash.",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem)
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;

        } else if let Some(&super::FakeClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {
            println!(
                "Call to faked class \"{}\" ({:?}) {} method \"{}\". Behaving as if message was sent to nil.",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem)
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        } else {
            println!(
                "SAFE BYPASS: Item {:?} in superclass chain of object {:?}'s class {:?} has an unexpected host object type. Returning 0 to prevent crash.",
                class, receiver, orig_class
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;
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
pub struct objc_super {
    pub receiver: id,
    pub class: Class,
}
unsafe impl SafeRead for objc_super {}

#[allow(non_snake_case)]
pub(super) fn objc_msgSendSuper2(
    env: &mut Environment,
    super_ptr: ConstPtr<objc_super>,
    selector: SEL,
) {
    let objc_super { receiver, class } = env.mem.read(super_ptr);
    crate::abi::write_next_arg(&mut 0, env.cpu.regs_mut(), &mut env.mem, receiver);

    objc_msgSend_inner(
        env,
        receiver,
        selector,
        /* super2: */ Some(class),
        /* tolerate_type_mismatch: */ false,
    )
}

pub trait MsgSendSignature: 'static {
    fn type_info() -> (TypeId, &'static str) {
        #[cfg(debug_assertions)]
        let type_name = std::any::type_name::<Self>();
        #[cfg(not(debug_assertions))]
        let type_name = "[description unavailable in release builds]";
        (TypeId::of::<Self>(), type_name)
    }
}

pub fn msg_send<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
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

pub trait MsgSendSuperSignature: 'static {
    type WithoutSuper: MsgSendSignature;
}

pub fn msg_send_super2<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSuperSignature,
    R: GuestRet,
{
    env.objc.message_type_info = Some(<(R, P) as MsgSendSuperSignature>::WithoutSuper::type_info());
    if R::SIZE_IN_MEM.is_some() {
        todo!() 
    } else {
        (objc_msgSendSuper2 as fn(&mut Environment, ConstPtr<objc_super>, SEL))
            .call_from_host(env, args)
    }
}

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
pub use crate::msg;

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
pub use crate::msg_super;

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
pub use crate::msg_class;

pub fn retain(env: &mut Environment, object: id) -> id {
    if object == nil { return nil; }
    msg![env; object retain]
}

pub fn release(env: &mut Environment, object: id) {
    if object == nil { return; }
    msg![env; object release]
}

pub fn autorelease(env: &mut Environment, object: id) -> id {
    if object == nil { return nil; }
    msg![env; object autorelease]
}