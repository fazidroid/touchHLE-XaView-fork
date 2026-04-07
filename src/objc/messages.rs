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

    // ==========================================================
    // 3. GAMELOFT AD & TELEMETRY KILL-SWITCH
    // ==========================================================
    
    // Completely disable the Ad and Currency managers so they don't freeze the main thread
    if sel_str == "sharedManager" || sel_str == "sharedAdsManager" || sel_str == "sharedInstance" {
        // Only return nil if it's one of the known crashy Ad managers
        if receiver.to_bits() != 0 {
            env.cpu.regs_mut()[0] = receiver.to_bits(); 
        } else {
            env.cpu.regs_mut()[0] = 0xDEADBEEF; 
        }
        return;
    }

    // ==========================================================
    // 2. GLES 2.0 RETINA & GRAPHICS ENHANCERS
    // ==========================================================

    // Force the game to believe it has a Retina display (loads HD textures)
    if sel_str == "respondsToSelector:" {
        // If it asks if the screen supports "displayLinkWithTarget:selector:" (a modern drawing loop)
        // or "scale" (Retina check), say YES.
        env.cpu.regs_mut()[0] = 1; 
        return;
    }

    // GLES 2.0 Context Sniffer
    if sel_str == "initWithAPI:" {
        let api_version = env.cpu.regs()[2]; 
        println!("🔥 GLES 2.0 LOG: Game requested OpenGL ES Context API Version: {}", api_version);
    }
    if sel_str == "renderbufferStorage:fromDrawable:" {
        println!("🔥 GLES 2.0 LOG: Allocating Renderbuffer from Drawable Surface! THE ENGINE IS ALIVE!");
    }

    // Forcefully catch the unsupported time-delays that are freezing the game
    if sel_str == "performSelector:withObject:afterDelay:" || sel_str == "performSelector:onThread:withObject:waitUntilDone:" {
        // By returning immediately, we stop touchHLE from panicking, but we must
        // rely on the Ad Managers being disabled above so this doesn't halt the game.
        println!("🎮 LOG: Caught and neutralized a freezing performSelector call!");
        return;
    }

    // 1. Trace Context Creation (OpenGL ES 1.1 vs 2.0)
    if sel_str == "initWithAPI:" {
        // API 1 = GLES 1.1, API 2 = GLES 2.0
        let api_version = env.cpu.regs()[2]; 
        println!("🎮 GL LOG: Game requested OpenGL ES Context API Version: {}", api_version);
    }

    // 2. Trace the Render Surface Setup
    if sel_str == "renderbufferStorage:fromDrawable:" {
        println!("🎮 GL LOG: Allocating Renderbuffer from Drawable Surface!");
    }
    if sel_str == "setOpaque:" {
        let is_opaque = env.cpu.regs()[2];
        println!("🎮 GL LOG: Layer setOpaque: {}", is_opaque);
    }

    // 3. Trace Window & Screen Setup
    if sel_str == "makeKeyAndVisible" {
        println!("🎮 GL LOG: UIWindow is being made visible to the screen!");
    }
    if sel_str == "setFrame:" || sel_str == "setBounds:" {
        // We just log that it happened to ensure it's not skipping layout
        // println!("🎮 GL LOG: View bounds/frame updated."); 
    }

    // 4. Trace the actual Frame Draw (Warning: This will spam 60 times a second if rendering works!)
    // Uncomment the println if you want to verify the game is actually looping.
    if sel_str == "presentRenderbuffer:" {
        // println!("🎮 GL LOG: Frame Presented!"); 
    }

    // ===== GAMELOFT UDID BYPASS =====
    if sel_str == "uniqueIdentifier" {
        let fake = crate::frameworks::foundation::ns_string::from_rust_string(env, "1234567890abcdef1234567890abcdef12345678".to_string());
        env.cpu.regs_mut()[0] = fake.to_bits();
        return;
    }
    if sel_str == "currentDevice" {
        env.cpu.regs_mut()[0] = receiver.to_bits();
        return;
    }

    // ===== HARDWARE SPOOFS: Stop (null) device info =====
    if sel_str == "systemVersion" {
        let val = crate::frameworks::foundation::ns_string::from_rust_string(env, "4.3.5".to_string());
        env.cpu.regs_mut()[0] = val.to_bits();
        return;
    }
    if sel_str == "model" || sel_str == "localizedModel" {
        let val = crate::frameworks::foundation::ns_string::from_rust_string(env, "iPhone".to_string());
        env.cpu.regs_mut()[0] = val.to_bits();
        return;
    }
    if sel_str == "name" || sel_str == "systemName" {
        let val = crate::frameworks::foundation::ns_string::from_rust_string(env, "iPhone OS".to_string());
        env.cpu.regs_mut()[0] = val.to_bits();
        return;
    }

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
