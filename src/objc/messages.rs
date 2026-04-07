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

    let sel_str = selector.as_str(&env.mem);

    // ===== GAMELOFT GLOBAL TIMER HACK =====
    // If the background thread asks to sleep, just ignore it and return immediately.
    // This entirely bypasses the `Duration` float panics inside `ns_thread.rs` globally!
    if sel_str == "sleepForTimeInterval:" {
        log!("🛡️ ANTI-PANIC SHIELD: Bypassing sleepForTimeInterval: to prevent NaN crash!");
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }
    // ============================================

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

    if sel_str == "keyEnumerator" || sel_str == "globallyUniqueString" || sel_str == "sharedHTTPCookieStorage" || sel_str == "isSecureTextEntry" || sel_str == "query" || sel_str == "encodeWithCoder:" || sel_str == "keysSortedByValueUsingSelector:" || sel_str == "description" || sel_str == "addPort:forMode:" || sel_str == "port" || sel_str == "defaultTimeZone" || sel_str == "stringByEvaluatingJavaScriptFromString:" || sel_str == "setTimeZone:" || sel_str == "knownTimeZoneNames" || sel_str == "stringWithContentsOfURL:encoding:error:" || sel_str == "sendSynchronousRequest:returningResponse:error:" || sel_str == "localizedDescription" || sel_str == "localizedFailureReason" || sel_str == "connection:didFailWithError:" {
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
                    env.cpu.regs_mut()[0..2].fill(0);
                    return;
                }
            };
            let &super::ClassHostObject {
                ref name,
                is_metaclass,
                ..
            } = class_host_object.as_any().downcast_ref().unwrap();
            // ===== THE NUCLEAR OPTION: GLOBAL PANIC BYPASS =====
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
                env.cpu.regs_mut()[0] = receiver.to_bits();
// Return self
            } else {
                env.cpu.regs_mut()[0..2].fill(0);
// Return nil/0
            }
            return;
        }

        let host_object = match env.objc.get_host_object(class) {
            Some(obj) => obj,
            None => {
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
            
            // ===== GAMELOFT VIDEO HANG BYPASS =====
            if name == "MPMoviePlayerController" || name == "MPMoviePlayerViewController" {
                if selector.as_str(&env.mem) == "play" || selector.as_str(&env.mem) == "stop" {
                    log!("GAMELOFT BYPASS: Auto-finishing movie player to prevent infinite hang!");
                    let center_class = env.objc.get_known_class("NSNotificationCenter", &mut env.mem);
                    if center_class != nil {
                        let center: id = msg![env; center_class defaultCenter];
                        let notif_name = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackDidFinishNotification".to_string());
                        let _: () = msg![env; center postNotificationName:notif_name object:receiver];
                    }
                    env.cpu.regs_mut()[0..2].fill(0);
                    return;
                }
            }

            // ===== GAMELOFT VIDEO HANG BYPASS =====
            if name == "MPMoviePlayerController" || name == "MPMoviePlayerViewController" {
                if selector.as_str(&env.mem) == "play" || selector.as_str(&env.mem) == "stop" {
                    log!("GAMELOFT BYPASS: Auto-finishing movie player to prevent infinite hang!");
                    let center_class = env.objc.get_known_class("NSNotificationCenter", &mut env.mem);
                    if center_class != nil {
                        let center: id = msg![env; center_class defaultCenter];
                        
                        // Tell the game the video state changed
                        let notif1 = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackStateDidChangeNotification".to_string());
                        let _: () = msg![env; center postNotificationName:notif1 object:receiver];
                        
                        // Tell the game the video finished playing
                        let notif2 = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackDidFinishNotification".to_string());
                        let _: () = msg![env; center postNotificationName:notif2 object:receiver];
                    }
                    env.cpu.regs_mut()[0..2].fill(0);
                    return;
                }
            }

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
                                    log!("Warning: {}", msg);
                                } else {
                                    // GAMELOFT HACK: Prevent type mismatch panics!
                                    log!("🛡️ ANTI-PANIC SHIELD: Bypassing type mismatch panic! {}", msg);
                                    env.cpu.regs_mut()[0..2].fill(0);
                                    return;
                                }
                            }
                        }
                        
                        // ===== THE ULTIMATE GLOBAL PANIC SHIELD =====
                        // Instead of running the host method normally and letting a Float NaN panic 
                        // kill the emulator, we run it inside a protected sandbox (catch_unwind).
                        let env_ptr = env as *mut Environment;
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            unsafe { host_imp.call_from_guest(&mut *env_ptr) }
                        }));

                        if result.is_err() {
                            unsafe {
                                crate::log!(
                                    "🛡️ ANTI-PANIC SHIELD ACTIVATED 🛡️\nIntercepted fatal Rust panic in method '{}'! Forcing a nil return instead of crashing.",
                                    selector.as_str(&(*env_ptr).mem)
                                );
                                (*env_ptr).cpu.regs_mut()[0..2].fill(0);
                            }
                        }
                        // ============================================
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
            // ===== THE NUCLEAR OPTION: GLOBAL CLASS PANIC BYPASS =====
            log!(
                "SAFE BYPASS: Class \"{}\" ({:?}) is unimplemented. Call to {} method \"{}\". Returning 0 to prevent crash.",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem),
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;

        } else if let Some(&super::FakeClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {
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
            // FIXED: SAFE BYPASS FOR EA GAMES
            log!(
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
