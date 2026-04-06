/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Foundation framework.
//!
//! A concept that Foundation really likes is "class clusters": abstract classes
//! with private concrete implementations. Apple has their own explanation of it
//! in [Cocoa Core Competencies](https://developer.apple.com/library/archive/documentation/General/Conceptual/DevPedia-CocoaCore/ClassCluster.html).
//! Being aware of this concept will make common types like `NSArray` and
//! `NSString` easier to understand.

use crate::dyld::{export_c_func, FunctionExports};
use crate::objc::id;
use crate::Environment;

pub mod _nib_archive_decoder;
pub mod ns_array;
pub mod ns_autorelease_pool;
pub mod ns_bundle;
pub mod ns_character_set;
pub mod ns_coder;
pub mod ns_data;
pub mod ns_date;
pub mod ns_date_formatter;
pub mod ns_dictionary;
pub mod ns_enumerator;
pub mod ns_error;
pub mod ns_exception;
pub mod ns_file_handle;
pub mod ns_file_manager;
pub mod ns_keyed_archiver;
pub mod ns_keyed_unarchiver;
pub mod ns_locale;
pub mod ns_lock;
pub mod ns_log;
pub mod ns_notification;
pub mod ns_notification_center;
pub mod ns_null;
pub mod ns_number;
pub mod ns_number_formatter;
pub mod ns_operation; // FIXED: NSOperation module added to expose background threads
pub mod ns_path_utilities;
pub mod ns_process_info;
pub mod ns_property_list;
pub mod ns_set;
pub mod ns_string;
pub mod ns_thread;
pub mod ns_timer;
pub mod ns_url;
pub mod ns_url_cache;
pub mod ns_url_connection;
pub mod ns_url_request;
pub mod ns_url_response;
pub mod ns_user_defaults;
pub mod ns_value;

use crate::objc::ClassExports;

pub const CLASSES: ClassExports = &[
    _nib_archive_decoder::CLASSES,
    ns_array::CLASSES,
    ns_autorelease_pool::CLASSES,
    ns_bundle::CLASSES,
    ns_character_set::CLASSES,
    ns_coder::CLASSES,
    ns_data::CLASSES,
    ns_date::CLASSES,
    ns_date_formatter::CLASSES,
    ns_dictionary::CLASSES,
    ns_enumerator::CLASSES,
    ns_error::CLASSES,
    ns_exception::CLASSES,
    ns_file_handle::CLASSES,
    ns_file_manager::CLASSES,
    ns_keyed_archiver::CLASSES,
    ns_keyed_unarchiver::CLASSES,
    ns_locale::CLASSES,
    ns_lock::CLASSES,
    ns_notification::CLASSES,
    ns_notification_center::CLASSES,
    ns_null::CLASSES,
    ns_number::CLASSES,
    ns_number_formatter::CLASSES,
    ns_operation::CLASSES, // FIXED: NSOperation classes exposed to the emulator
    ns_process_info::CLASSES,
    ns_set::CLASSES,
    ns_string::CLASSES,
    ns_thread::CLASSES,
    ns_timer::CLASSES,
    ns_url::CLASSES,
    ns_url_cache::CLASSES,
    ns_url_connection::CLASSES,
    ns_url_request::CLASSES,
    ns_url_response::CLASSES,
    ns_user_defaults::CLASSES,
    ns_value::CLASSES,
];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSLog(_, _)),
    export_c_func!(NSLogv(_, _, _)),
    export_c_func!(NSHomeDirectory()),
    export_c_func!(NSTemporaryDirectory()),
    export_c_func!(NSSearchPathForDirectoriesInDomains(_, _, _)),
    export_c_func!(NSStringFromRange(_)),
];

fn NSLog(env: &mut Environment, format: id, args: crate::abi::DotDotDot) {
    ns_log::ns_log(env, format, args);
}

fn NSLogv(env: &mut Environment, format: id, args: crate::mem::MutVoidPtr, _args2: crate::abi::DotDotDot) {
    ns_log::ns_logv(env, format, args);
}

fn NSHomeDirectory(env: &mut Environment) -> id {
    ns_path_utilities::ns_home_directory(env)
}

fn NSTemporaryDirectory(env: &mut Environment) -> id {
    ns_path_utilities::ns_temporary_directory(env)
}

fn NSSearchPathForDirectoriesInDomains(
    env: &mut Environment,
    directory: NSUInteger,
    domain_mask: NSUInteger,
    expand_tilde: bool,
) -> id {
    ns_path_utilities::ns_search_path_for_directories_in_domains(
        env,
        directory,
        domain_mask,
        expand_tilde,
    )
}

pub type NSInteger = i32;
pub type NSUInteger = u32;

#[repr(C, packed)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct NSRange {
    pub location: NSUInteger,
    pub length: NSUInteger,
}
unsafe impl crate::mem::SafeRead for NSRange {}
impl crate::abi::GuestArg for NSRange {
    const REGISTER_COUNT: usize = 2;
    fn from_regs(regs: &[u32]) -> Self {
        NSRange {
            location: crate::abi::GuestArg::from_regs(&regs[0..1]),
            length: crate::abi::GuestArg::from_regs(&regs[1..2]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.location.to_regs(&mut regs[0..1]);
        self.length.to_regs(&mut regs[1..2]);
    }
}

fn NSStringFromRange(env: &mut Environment, range: NSRange) -> id {
    let loc = range.location;
    let len = range.length;
    let string = format!("{{{loc}, {len}}}");
    ns_string::from_rust_string(env, string)
}

pub type NSComparisonResult = NSInteger;
pub const NSOrderedAscending: NSComparisonResult = -1;
pub const NSOrderedSame: NSComparisonResult = 0;
pub const NSOrderedDescending: NSComparisonResult = 1;

/// Number of seconds.
pub type NSTimeInterval = f64;

/// UTF-16 code unit.
#[allow(non_camel_case_types)]
pub type unichar = u16;

/// Utility to help with implementing the `hash` method, which various classes
/// in Foundation have to do.
fn hash_helper<T: std::hash::Hash>(hashable: &T) -> NSUInteger {
    use std::hash::Hasher;

    // Rust documentation says DefaultHasher::new() should always return the
    // same instance, so this should give consistent hashes.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hashable.hash(&mut hasher);
    hasher.finish() as NSUInteger
}
