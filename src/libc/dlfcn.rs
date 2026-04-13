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
    // TODO: dlopen() support for real dynamic libraries.
    assert!(is_known_library(env.mem.cstr_at_utf8(path).unwrap()));
    // For convenience, use the path as the handle.
    // TODO: Find out whether the handle is truly opaque on iPhone OS, and if
    // not, where it points.
    path.cast_mut().cast()
}

fn dlsym(env: &mut Environment, handle: MutVoidPtr, symbol: ConstPtr<u8>) -> MutVoidPtr {
    assert!(
        handle == RTLD_DEFAULT || is_known_library(env.mem.cstr_at_utf8(handle.cast()).unwrap())
    );
    // For some reason, the symbols passed to dlsym() don't have the leading _.
    let symbol = format!("_{}", env.mem.cstr_at_utf8(symbol).unwrap());

    // ==========================================================
    // 🏎️ ASPHALT 8 BYPASS: Gameloft DRM Integrity Check
    // ==========================================================
    if symbol == "_main" {
        println!("🎮 LOG: Defeating Gameloft DRM main() check!");
        // Give the DRM the actual memory pointer to Asphalt 8's entry point
        // so it can scan the assembly and pass the anti-crack check!
        return crate::mem::Ptr::from_bits(0x0000aef8); 
    }

    // TODO: error handling. dlsym() should just return NULL in this case, but
    // currently it's probably more useful to have the emulator crash if there's
    // no symbol found, since it most likely indicates a missing host function.
    // TODO: Symbol lookup should be scoped to the specific library requested,
    // where appropriate!
    let addr = env
        .dyld
        .create_proc_address(&mut env.mem, &mut env.cpu, &symbol)
        .unwrap_or_else(|_| panic!("dlsym() for unimplemented function {symbol}"));
    Ptr::from_bits(addr.addr_with_thumb_bit())
}

fn dlclose(env: &mut Environment, handle: MutVoidPtr) -> i32 {
    assert!(
        handle == RTLD_DEFAULT || is_known_library(env.mem.cstr_at_utf8(handle.cast()).unwrap())
    );
    0 // success
}

// ==========================================================
// 🏎️ ASPHALT 8 BYPASS: dladdr() Gameloft DRM Defeater
// ==========================================================
#[repr(C, packed)]
pub struct Dl_info {
    pub dli_fname: crate::mem::ConstPtr<u8>,
    pub dli_fbase: crate::mem::MutVoidPtr,
    pub dli_sname: crate::mem::ConstPtr<u8>,
    pub dli_saddr: crate::mem::MutVoidPtr,
}
unsafe impl crate::mem::SafeRead for Dl_info {}

fn dladdr(env: &mut Environment, addr: crate::mem::ConstVoidPtr, info_ptr: crate::mem::MutPtr<Dl_info>) -> i32 {
    if info_ptr.is_null() { return 0; }

    println!("🎮 LOG: Gameloft dladdr() DRM Defeated! Address: {:#x}", addr.to_bits());

    // 1. Get the current Stack Pointer (SP)
    let sp = env.cpu.regs()[13];
    
    // 2. Safely point to the "Red Zone" (unused space below the stack)
    let fake_str_ptr: crate::mem::ConstPtr<u8> = crate::mem::Ptr::from_bits(sp - 64);
    
    // 3. Write "main\0" directly into the guest's memory
    env.mem.bytes_at_mut(fake_str_ptr.cast_mut(), 5).copy_from_slice(b"main\0");

    // 4. Hand the fake memory pointer back to Gameloft's Anti-Piracy checker
    let info = Dl_info {
        dli_fname: fake_str_ptr, 
        dli_fbase: crate::mem::Ptr::from_bits(0x4000), // Standard iOS binary base
        dli_sname: fake_str_ptr, // <--- This satisfies the main() string check!
        dli_saddr: addr.cast_mut(),
    };

    env.mem.write(info_ptr, info);
    1 // Return 1 for Success
}

// Export the newly added dladdr function so the game can call it
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(dlopen(_, _)),
    export_c_func!(dlsym(_, _)),
    export_c_func!(dlclose(_)),
    export_c_func!(dladdr(_, _)),
];
