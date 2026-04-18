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

/// Opaque type representing a SQLite database connection
#[repr(C)]
pub struct sqlite3 {
    _private: [u8; 0],
}

fn sqlite3_open(
    env: &mut Environment,
    filename: ConstPtr<u8>,
    pp_db: MutPtr<*mut sqlite3>,
) -> i32 {
    let path = if filename.is_null() {
        ":memory:".to_string()
    } else {
        env.mem.cstr_at_utf8(filename).unwrap_or_default().to_string()
    };
    log!("sqlite3_open: filename = {:?}", path);

    let dummy_db = Box::into_raw(Box::new(sqlite3 { _private: [] }));
    env.mem.write(pp_db, dummy_db);

    SQLITE_OK
}

fn sqlite3_close(_env: &mut Environment, db: *mut sqlite3) -> i32 {
    log!("sqlite3_close: {:?}", db);
    if !db.is_null() {
        unsafe { drop(Box::from_raw(db)) };
    }
    SQLITE_OK
}

fn sqlite3_exec(
    _env: &mut Environment,
    _db: *mut sqlite3,
    _sql: ConstPtr<u8>,
    _callback: ConstVoidPtr,
    _arg: ConstVoidPtr,
    _errmsg: MutPtr<ConstPtr<u8>>,
) -> i32 {
    log!("sqlite3_exec stub: returning SQLITE_OK");
    SQLITE_OK
}

fn sqlite3_errmsg(_env: &mut Environment, _db: *mut sqlite3) -> ConstPtr<u8> {
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
