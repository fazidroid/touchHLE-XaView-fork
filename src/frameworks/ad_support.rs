// src/frameworks/ad_support.rs
use crate::dyld::HostDylib;

// This pulls in your fixed as_identifier_manager.rs file
pub mod as_identifier_manager;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/AdSupport.framework/AdSupport",
    aliases: &[],
    // This now exports the classes from the sub-module
    class_exports: &[as_identifier_manager::CLASSES], 
    function_exports: &[],
    constant_exports: &[],
};
