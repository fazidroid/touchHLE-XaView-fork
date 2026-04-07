/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C messaging (`objc_msgSend` and friends).

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
    log_dbg!("Dispatching {} for {:?}", selector.as_str(&env.mem), receiver);
    let message_type_info = env.objc.message_type_info.take();
    let sel_str = selector.as_str(&env.mem);

    // ===== NFS MOST WANTED MTX / STOREFRONT BYPASS =====
    // 1. Pretend the MTX controller is always ready and authorized
    if sel_str == "canMakePayments" {
        env.cpu.regs_mut()[0] = 1; // Return YES
        return;
    }
    if sel_str == "isMTXReady" || sel_str == "CheckMTXController" {
        log!("EA BYPASS: Forcing MTX Controller status to READY");
        env.cpu.regs_mut()[0] = 1; // 1 = true
        return;
    }

    // 3. Return a dummy object for the Storefront controller if it asks for a singleton
    if sel_str == "sharedController" || sel_str == "defaultQueue" {
        log!("EA BYPASS: Providing dummy object for {}", sel_str);
        // Return the receiver itself (or any non-zero pointer) so the game thinks the object exists
        env.cpu.regs_mut()[0] = receiver.to_bits();
        return;
    }
    // 2. Bypass MTX initialization check
    if sel_str == "CheckMTXController" || sel_str == "isMTXReady" {
        env.cpu.regs_mut()[0] = 1; // Return true
        return;
    }

    // 3. Prevent crash when the engine looks for the 'Main' Window during MTX setup
    if sel_str == "keyWindow" {
        // Return the receiver if it's likely a UIWindow, or nil if unsure
        env.cpu.regs_mut()[0] = receiver.to_bits(); 
        return;
    }

    // ===== GAMELOFT UDID & DEVICE BYPASS =====
    
    if sel_str == "uniqueIdentifier" {
        let fake = crate::frameworks::foundation::ns_string::from_rust_string(env, "1234567890abcdef1234567890abcdef12345678".to_string());
        env.cpu.regs_mut()[0] = fake.to_bits();
        return;
    }
    if sel_str == "currentDevice" {
        env.cpu.regs_mut()[0] = receiver.to_bits();
        return;
    }

    // ===== GAMELOFT UDID & DEVICE BYPASS =====
    if sel_str == "uniqueIdentifier" {
        let fake = crate::frameworks::foundation::ns_string::from_rust_string(env, "1234567890abcdef1234567890abcdef12345678".to_string());
        env.cpu.regs_mut()[0] = fake.to_bits();
        return;
    }
    if sel_str == "currentDevice" {
        env.cpu.regs_mut()[0] = receiver.to_bits();
        return;
    }
    // NEW: Stop Most Wanted from getting (null) device info
    if sel_str == "name" {
        let fake = crate::frameworks::foundation::ns_string::from_rust_string(env, "iPhone".to_string());
        env.cpu.regs_mut()[0] = fake.to_bits();
        return;
    }
    if sel_str == "systemName" {
        let fake = crate::frameworks::foundation::ns_string::from_rust_string(env, "iPhone OS".to_string());
        env.cpu.regs_mut()[0] = fake.to_bits();
        return;
    }
    if sel_str == "systemVersion" {
        let fake = crate::frameworks::foundation::ns_string::from_rust_string(env, "4.3.5".to_string());
        env.cpu.regs_mut()[0] = fake.to_bits();
        return;
    }
    if sel_str == "model" {
        let fake = crate::frameworks::foundation::ns_string::from_rust_string(env, "iPhone".to_string());
        env.cpu.regs_mut()[0] = fake.to_bits();
        return;
    }

    // ===== URL Tracker & Telemetry Bypasses =====
    if sel_str == "HTTPMethod" || sel_str == "host" || sel_str == "addValue:forHTTPHeaderField:" || sel_str == "setValue:forHTTPHeaderField:" {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    if receiver == nil {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let orig_class = super2.unwrap_or_else(|| ObjC::read_isa(receiver, &env.mem));
    if orig_class == nil {
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let mut class = orig_class;
    loop {
        if class == nil {
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }

        // --- SCOPED PRE-CHECK BYPASSES ---
        let mut do_video_bypass = false;
        let mut do_alert_bypass = false;

        { 
            if let Some(host_object) = env.objc.get_host_object(class) {
                if let Some(obj) = host_object.as_any().downcast_ref::<super::ClassHostObject>() {
                    let name = &obj.name;
                    if (name == "MPMoviePlayerController" || name == "MPMoviePlayerViewController") && (sel_str == "play" || sel_str == "stop") {
                        do_video_bypass = true;
                    }
                    if name == "UIAlertView" && sel_str == "show" {
                        do_alert_bypass = true;
                    }
                }
            }
        } 

        if do_video_bypass {
            log!("GAMELOFT BYPASS: Skipping video player hang.");
            let center_class = env.objc.get_known_class("NSNotificationCenter", &mut env.mem);
            if center_class != nil {
                let center: id = msg![env; center_class defaultCenter];
                let n1 = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackStateDidChangeNotification".to_string());
                let _: () = msg![env; center postNotificationName:n1 object:receiver];
                let n2 = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackDidFinishNotification".to_string());
                let _: () = msg![env; center postNotificationName:n2 object:receiver];
            }
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }

        if do_alert_bypass {
            log!("AUTO-DISMISSING UIAlertView.");
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }

        // --- NORMAL METHOD LOOKUP ---
        let host_object = match env.objc.get_host_object(class) {
            Some(obj) => obj,
            None => { env.cpu.regs_mut()[0..2].fill(0); return; }
        };

        if let Some(obj) = host_object.as_any().downcast_ref::<super::ClassHostObject>() {
            let superclass = obj.superclass;
            if super2.is_some() && class == orig_class {
                class = superclass;
                continue;
            }

            if let Some(imp) = obj.methods.get(&selector) {
                match imp {
                    IMP::Host(host_imp) => {
                        if let Some((sent_type_id, _)) = message_type_info {
                            let (expected_type_id, _) = host_imp.type_info();
                            if sent_type_id != expected_type_id && !tolerate_type_mismatch && sel_str != "bytes" && sel_str != "length" {
                                panic!("Type mismatch for {}!", sel_str);
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
        } else {
            // UNKNOWN HOST OBJECT / SAFE BYPASS
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }
    }
}

// Boilerplate below is unchanged...
#[allow(non_snake_case)]
pub(super) fn objc_msgSend(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, false)
}
#[allow(non_snake_case)]
pub(crate) fn _touchHLE_objc_msgSend_tolerant(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, true)
}
pub(super) fn objc_msgSend_stret(env: &mut Environment, _stret: MutVoidPtr, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, false)
}
#[allow(non_snake_case)]
pub(crate) fn _touchHLE_objc_msgSend_stret_tolerant(env: &mut Environment, _stret: MutVoidPtr, receiver: id, selector: SEL) {
    objc_msgSend_inner(env, receiver, selector, None, true)
}
#[repr(C, packed)]
pub struct objc_super { pub receiver: id, pub class: Class }
unsafe impl SafeRead for objc_super {}
#[allow(non_snake_case)]
pub(super) fn objc_msgSendSuper2(env: &mut Environment, super_ptr: ConstPtr<objc_super>, selector: SEL) {
    let objc_super { receiver, class } = env.mem.read(super_ptr);
    crate::abi::write_next_arg(&mut 0, env.cpu.regs_mut(), &mut env.mem, receiver);
    objc_msgSend_inner(env, receiver, selector, Some(class), false)
}
pub trait MsgSendSignature: 'static {
    fn type_info() -> (TypeId, &'static str) {
        (TypeId::of::<Self>(), std::any::type_name::<Self>())
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
        (_touchHLE_objc_msgSend_stret_tolerant as fn(&mut Environment, MutVoidPtr, id, SEL)).call_from_host(env, args)
    } else {
        (_touchHLE_objc_msgSend_tolerant as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}
pub trait MsgSendSuperSignature: 'static { type WithoutSuper: MsgSendSignature; }
pub fn msg_send_super2<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSuperSignature,
    R: GuestRet,
{
    env.objc.message_type_info = Some(<(R, P) as MsgSendSuperSignature>::WithoutSuper::type_info());
    if R::SIZE_IN_MEM.is_some() { todo!() } 
    else { (objc_msgSendSuper2 as fn(&mut Environment, ConstPtr<objc_super>, SEL)).call_from_host(env, args) }
}
#[macro_export]
macro_rules! msg {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel).expect("Unknown selector");
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
            let class = $env.objc.get_known_class(_OBJC_CURRENT_CLASS, &mut $env.mem);
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel).expect("Unknown selector");
            let sp = &mut $env.cpu.regs_mut()[$crate::cpu::Cpu::SP];
            let old_sp = *sp;
            *sp -= $crate::mem::guest_size_of::<$crate::objc::objc_super>();
            let super_ptr = $crate::mem::Ptr::from_bits(*sp);
            $env.mem.write(super_ptr, $crate::objc::objc_super { receiver: $receiver, class });
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
            let class = $env.objc.get_known_class(stringify!($receiver_class), &mut $env.mem);
            $crate::objc::msg![$env; class $name $(: $arg1 $($($namen)?: $argn)*)?]
        }
    }
}
pub use crate::msg_class;
pub fn retain(env: &mut Environment, object: id) -> id { if object == nil { return nil; } msg![env; object retain] }
pub fn release(env: &mut Environment, object: id) { if object == nil { return; } msg![env; object release] }
pub fn autorelease(env: &mut Environment, object: id) -> id { if object == nil { return nil; } msg![env; object autorelease] }
