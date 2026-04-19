/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Captive Network API stubs.

use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;

// ==========================================================
// 🏎️ GT RACING 2 BYPASS: Network Interface Stub
// ==========================================================
fn CNCopySupportedInterfaces(_env: &mut Environment) -> u32 {
    println!("🎮 LOG: Safely stubbed CNCopySupportedInterfaces! Returning NULL.");
    0 
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CNCopySupportedInterfaces(_)),
];
