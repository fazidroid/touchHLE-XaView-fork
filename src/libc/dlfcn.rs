/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `dlfcn.h` (`dlopen()` and friends)

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, MutVoidPtr, Ptr};
use crate::Environment;

const RTLD_DEFAULT: MutVoidPtr = Ptr::from_bits(-2 as _);

fn is_known_library(path: &str) -> bool {
    crate::dyld::DYLIB_LIST
        .iter()
        .any(|dylib| dylib.path == path || dylib.aliases.contains(&path))
}

fn dlopen(env: &mut Environment, path: ConstPtr<u8>, _mode: i32) -> MutVoidPtr {
    if path.is_null() {
        return RTLD_DEFAULT;
    }
    
    // 🛡️ BORROW CHECKER BYPASS: Convert to owned String immediately!
    let path_str = env.mem.cstr_at_utf8(path).unwrap().to_string();
    
    // 🛡️ THE PHANTOM STOREKIT BYPASS
    // If EA looks for StoreKit, intercept it and manually inject the classes into the runtime!
    if path_str.contains("StoreKit") {
        println!("🛡️ DLOPEN BYPASS: Intercepted StoreKit! Injecting Phantom Classes...");
        env.objc.link_class("SKPaymentQueue", false, &mut env.mem);
        env.objc.link_class("SKProductsRequest", false, &mut env.mem);
        env.objc.link_class("SKPayment", false, &mut env.mem);
        env.objc.link_class("SKMutablePayment", false, &mut env.mem);
        return path.cast_mut().cast(); // Return valid fake handle
    }
    
    assert!(is_known_library(&path_str));
    path.cast_mut().cast()
}

fn dlsym(env: &mut Environment, handle: MutVoidPtr, symbol: ConstPtr<u8>) -> MutVoidPtr {
    let handle_str = if handle == RTLD_DEFAULT { 
        String::new() 
    } else { 
        env.mem.cstr_at_utf8(handle.cast()).unwrap_or("").to_string() 
    };

    let sym_str = env.mem.cstr_at_utf8(symbol).unwrap().to_string();

    // 🛡️ DLSYM SAFE NULL: Silently absorb missing StoreKit functions without crashing!
    if handle_str.contains("StoreKit") || sym_str.contains("SK") || sym_str.contains("StoreKit") {
        println!("🛡️ DLSYM BYPASS: Absorbed missing StoreKit symbol: {}", sym_str);
        return crate::mem::Ptr::from_bits(0); // Safe NULL pointer
    }

    assert!(
        handle == RTLD_DEFAULT || is_known_library(&handle_str)
    );
    let symbol_fmt = format!("_{}", sym_str);
    
    let addr = env
        .dyld
        .create_proc_address(&mut env.mem, &mut env.cpu, &symbol_fmt)
        .unwrap_or_else(|_| panic!("dlsym() for unimplemented function {symbol_fmt}"));
    Ptr::from_bits(addr.addr_with_thumb_bit())
}

fn dlclose(env: &mut Environment, handle: MutVoidPtr) -> i32 {
    let handle_str = if handle == RTLD_DEFAULT { 
        String::new() 
    } else { 
        env.mem.cstr_at_utf8(handle.cast()).unwrap_or("").to_string() 
    };
    
    if handle_str.contains("StoreKit") {
        return 0; // Fake success
    }

    assert!(
        handle == RTLD_DEFAULT || is_known_library(&handle_str)
    );
    0 // success
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(dlopen(_, _)),
    export_c_func!(dlsym(_, _)),
    export_c_func!(dlclose(_)),
];
            
