/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFNumber`.
//!
//! This is toll-free bridged to `NSNumber` in Apple's implementation.
//! Here it is the same type.

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::{CFIndex, CFTypeRef};
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::ConstVoidPtr;
use crate::objc::{msg, msg_class};
use crate::Environment;

type CFNumberType = CFIndex;
const kCFNumberSInt32Type: CFNumberType = 3;
const kCFNumberCharType: CFNumberType = 7;
const kCFNumberShortType: CFNumberType = 8;
const kCFNumberFloatType: CFNumberType = 12;

type CFNumberRef = CFTypeRef;

fn CFNumberCreate(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    type_: CFNumberType,
    value_ptr: ConstVoidPtr,
) -> CFNumberRef {
    // TODO: unique some common numbers to improve performance
    assert_eq!(allocator, kCFAllocatorDefault); // unimplemented
    log_dbg!("CFNumberCreate type {}", type_);
    let num = msg_class![env; NSNumber alloc];
    match type_ {
        kCFNumberSInt32Type => {
            let val: i32 = env.mem.read(value_ptr.cast());
            msg![env; num initWithInt:val]
        }
        kCFNumberCharType => {
            let val: i8 = env.mem.read(value_ptr.cast());
            msg![env; num initWithChar:val]
        }
        kCFNumberShortType => {
            let val: i16 = env.mem.read(value_ptr.cast());
            msg![env; num initWithShort:val]
        }
        kCFNumberFloatType => {
            let val: f32 = env.mem.read(value_ptr.cast());
            msg![env; num initWithFloat:val]
        }
        _ => unimplemented!("type {}", type_),
    }
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(CFNumberCreate(_, _, _))];
