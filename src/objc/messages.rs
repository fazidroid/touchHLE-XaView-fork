 &'static str) {

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
