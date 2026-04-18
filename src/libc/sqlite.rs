/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SQLite3 stub functions

use crate::dyld::{export_c_func, FunctionExports, HostDylib};
use crate::mem::{ConstPtr, MutPtr, Ptr};
use crate::Environment;

// SQLite result codes
const SQLITE_OK: i32 = 0;
const SQLITE_ERROR: i32 = 1;

// We represent sqlite3* as an opaque u32 guest pointer.
// The guest will treat it as a pointer to an sqlite3 struct.

fn sqlite3_open(
    env: &mut Environment,
    filename: ConstPtr<u8>,
    pp_db: MutPtr<u32>, // sqlite3** in guest memory
) -> i32 {
    let path = if filename.is_null() {
        ":memory:".to_string()
    } else {
        env.mem.cstr_at_utf8(filename).unwrap_or_default().to_string()
    };
    log!("sqlite3_open: filename = {:?}", path);

    // Allocate a dummy sqlite3 object in guest memory (just a non‑null address)
    // The real implementation would allocate a proper struct, but we just need a sentinel.
    let dummy_db = env.mem.alloc(1); // allocate 1 byte, just to have a non‑null address
    let dummy_db_ptr: u32 = dummy_db.to_bits();

    // Write the handle to the output pointer
    env.mem.write(pp_db, dummy_db_ptr);

    SQLITE_OK
}

fn sqlite3_close(env: &mut Environment, db: u32) -> i32 {
    log!("sqlite3_close: {:08x}", db);
    if db != 0 {
        // Free the dummy allocation (optional, we could leak it)
        let ptr: MutPtr<u8> = Ptr::from_bits(db);
        env.mem.free(ptr);
    }
    SQLITE_OK
}

fn sqlite3_exec(
    _env: &mut Environment,
    _db: u32,
    _sql: ConstPtr<u8>,
    _callback: u32, // function pointer
    _arg: u32,      // void* argument
    _errmsg: MutPtr<u32>, // char**
) -> i32 {
    log!("sqlite3_exec stub: returning SQLITE_OK");
    SQLITE_OK
}

fn sqlite3_errmsg(_env: &mut Environment, _db: u32) -> ConstPtr<u8> {
    // Return a dummy non‑null pointer to an empty string
    // In a real implementation we'd return a static string.
    Ptr::from_bits(0xdeadbeef)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sqlite3_open(_, _)),
    export_c_func!(sqlite3_close(_)),
    export_c_func!(sqlite3_exec(_, _, _, _, _)),
    export_c_func!(sqlite3_errmsg(_)),
];

pub const DYLIB: HostDylib = HostDylib {
    path: "/usr/lib/libsqlite3.dylib",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[FUNCTIONS],
};
