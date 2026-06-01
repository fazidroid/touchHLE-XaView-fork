/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Objective‑C exception handling (for `@try/@catch`).

use crate::objc::id;
use crate::Environment;

thread_local! {
    static CATCH_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Called when an Objective‑C exception is thrown.
#[no_mangle]
pub fn objc_exception_throw(_env: &mut Environment, exception: id) {
    log!("Objective‑C exception thrown: {:?}", exception);
    panic!("uncaught Objective‑C exception");
}

/// Called at the start of a `@catch` block. Returns the exception object.
#[no_mangle]
pub fn objc_begin_catch(_env: &mut Environment, exception: id) -> id {
    log_dbg!("objc_begin_catch({:?})", exception);
    CATCH_ACTIVE.with(|active| {
        assert!(!active.replace(true), "nested @catch not supported");
    });
    exception
}

/// Called at the end of a `@catch` block.
#[no_mangle]
pub fn objc_end_catch(_env: &mut Environment) {
    log_dbg!("objc_end_catch");
    CATCH_ACTIVE.with(|active| {
        assert!(active.replace(false), "objc_end_catch without active catch");
    });
}