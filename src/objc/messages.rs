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
    let sel_str = selector.as_str(&env.mem);
    let message_type_info = env.objc.message_type_info.take();
    // ==========================================================
    // TARGETED RETINA DISPLAY SPOOF (GLES 2.0 HD Textures)
    // ==========================================================

    // 1. Force the screen scale to 2.0 (Retina)
    // When the game asks [UIScreen mainScreen] for its scale, we give it 2.0 instead of 1.0.
    if sel_str == "scale" {
        // Return 2.0 as an IEEE 754 32-bit float in hex
        env.cpu.regs_mut()[0] = 0x40000000; 
        return;
    }

    // 2. Safely tell the game we support Retina features ONLY
    if sel_str == "respondsToSelector:" {
        // The selector the game is asking about is passed in the second argument (regs[2])
        let target_sel = Selector::from_bits(env.cpu.regs()[2]);
        let target_sel_str = target_sel.as_str(&env.mem);
        
        // ONLY say YES (1) if it's specifically asking about modern screen features
        if target_sel_str == "scale" || target_sel_str == "displayLinkWithTarget:selector:" {
            env.cpu.regs_mut()[0] = 1; 
            return;
        }
        
        // CRITICAL: If it asks about anything else (like Ad Manager views), 
        // we DO NOT return here. We let the code fall through to the normal emulator logic below!
    }

    // ==========================================================
    // 1. GRAPHICS & OPENGL SNIFFER (GLES 2.0)
    // ==========================================================

    if sel_str == "initWithAPI:" {
        let api_version = env.cpu.regs()[2]; 
        println!("🔥 GLES 2.0 LOG: Game requested OpenGL ES API Version: {}", api_version);
    }
    if sel_str == "renderbufferStorage:fromDrawable:" {
        println!("🔥 GLES 2.0 LOG: Allocating Renderbuffer! 3D ENGINE IS ALIVE!");
    }

    // ==========================================================
    // 2. NETWORK & AD KILL-SWITCH (Fixes the Loading Freeze)
    // ==========================================================
    
    // Kill Network Connections (Prevents hanging on Gameloft Live)
    if sel_str == "connectionWithRequest:delegate:" || 
       sel_str == "initWithRequest:delegate:" || 
       sel_str == "sendSynchronousRequest:returningResponse:error:" {
        env.cpu.regs_mut()[0] = 0; // Return nil
        return;
    }

    // Disable Ad Managers safely
    if sel_str == "sharedManager" || sel_str == "sharedAdsManager" {
        env.cpu.regs_mut()[0] = if receiver.to_bits() != 0 { receiver.to_bits() } else { 0xDEADBEEF }; 
        return;
    }

    // Neutralize thread-blocking delay loops
    if sel_str == "performSelector:withObject:afterDelay:" || sel_str == "performSelector:onThread:withObject:waitUntilDone:" {
        println!("🎮 LOG: Caught and neutralized a freezing performSelector call!");
        return;
    }

    // ==========================================================
    // 3. GAMELOFT IDENTITY & HARDWARE SPOOFS
    // ==========================================================
    
    if sel_str == "uniqueIdentifier" {
        let val = crate::frameworks::foundation::ns_string::from_rust_string(env, "1234567890abcdef1234567890abcdef12345678".to_string());
        env.cpu.regs_mut()[0] = val.to_bits();
        return;
    }

    if sel_str == "currentDevice" {
        env.cpu.regs_mut()[0] = if receiver.to_bits() != 0 { receiver.to_bits() } else { 0xDEADBEEF };
        return;
    }

    if sel_str == "name" || sel_str == "systemName" || sel_str == "model" || sel_str == "localizedModel" {
        let name = if sel_str == "systemName" { "iPhone OS" } else { "iPhone" };
        let val = crate::frameworks::foundation::ns_string::from_rust_string(env, name.to_string());
        env.cpu.regs_mut()[0] = val.to_bits();
        return;
    }

    if sel_str == "systemVersion" {
        let val = crate::frameworks::foundation::ns_string::from_rust_string(env, "4.3.5".to_string());
        env.cpu.regs_mut()[0] = val.to_bits();
        return;
    }

    // ==========================================================
    // 4. CORE DISPATCH LOGIC (With Recursion Fixes)
    // ==========================================================

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
            if sel_str == "self" { env.cpu.regs_mut()[0] = receiver.to_bits(); } 
            else { env.cpu.regs_mut()[0..2].fill(0); }
            return;
        }

        let host_object = match env.objc.get_host_object(class) {
            Some(obj) => obj,
            None => { env.cpu.regs_mut()[0..2].fill(0); return; }
        };

        if let Some(obj) = host_object.as_any().downcast_ref::<super::ClassHostObject>() {
            
            if super2.is_some() && class == orig_class {
                class = obj.superclass;
                continue;
            }

            let name = &obj.name;

            // --- GAMELOFT VIDEO PLAYER BYPASS ---
            if (name == "MPMoviePlayerController" || name == "MPMoviePlayerViewController") && (sel_str == "play" || sel_str == "stop") {
                let center_class = env.objc.get_known_class("NSNotificationCenter", &mut env.mem);
                if center_class != nil {
                    let center: id = msg![env; center_class defaultCenter];
                    let n = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackDidFinishNotification".to_string());
                    let _: () = msg![env; center postNotificationName:n object:receiver];
                }
                env.cpu.regs_mut()[0] = 0;
                return;
            }

            // --- NORMAL METHOD EXECUTION ---
            if let Some(imp) = obj.methods.get(&selector) {
                match imp {
                    IMP::Host(host_imp) => {
                        if let Some((sent_type_id, _)) = message_type_info {
                            let (expected_type_id, _) = host_imp.type_info();
                            if sent_type_id != expected_type_id && !tolerate_type_mismatch && 
                               sel_str != "bytes" && sel_str != "length" {
                                panic!("Type mismatch for {}!", sel_str);
                            }
                        }
                        host_imp.call_from_guest(env)
                    }
                    IMP::Guest(guest_imp) => guest_imp.call_without_pushing_stack_frame(env),
                }
                return;
            } else {
                class = obj.superclass;
            }
        } else {
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        }
    }
}

// Boilerplate below is unchanged
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
