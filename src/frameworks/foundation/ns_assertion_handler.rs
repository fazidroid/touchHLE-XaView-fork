/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::objc::{id, ClassExports, SEL};

pub const CLASSES: ClassExports = &[
    (
        "NSAssertionHandler",
        vec![
            // Class methods
            sel!(currentHandler => current_handler),
            // Instance methods
            sel!(handleFailureInMethod:object:file:lineNumber:description: => handle_failure),
            sel!(handleFailureInFunction:file:lineNumber:description: => handle_failure_func),
        ],
        None,
    ),
];

fn current_handler(this: id, _sel: SEL) -> id {
    log_dbg!("NSAssertionHandler currentHandler called, returning dummy object");
    this
}

fn handle_failure(
    _this: id,
    _sel: SEL,
    _method: id,
    _object: id,
    _file: id,
    _line: i32,
    _description: id,
) {
    log_once!("NSAssertionHandler handleFailureInMethod:... ignored");
}

fn handle_failure_func(
    _this: id,
    _sel: SEL,
    _function: id,
    _file: id,
    _line: i32,
    _description: id,
) {
    log_once!("NSAssertionHandler handleFailureInFunction:... ignored");
}
