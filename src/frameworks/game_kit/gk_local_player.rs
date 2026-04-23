/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `GKLocalPlayer`.

use crate::dyld::{ConstantExports, HostConstant};
use crate::objc::{id, objc_classes, ClassExports};
use crate::msg;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// TODO: proper inheritance chain
@implementation GKLocalPlayer: NSObject

+ (id)localPlayer {
        println!("🎮 LOG: Caught [GKLocalPlayer localPlayer]. Returning fake player!");
        let player: id = msg![env; this alloc];
        let player: id = msg![env; player init];
        crate::objc::autorelease(env, player)
    }

    // 🏎️ PROACTIVE STUBS: Game Center Auth Bypasses
    - (bool)isAuthenticated {
        println!("🎮 LOG: Caught [GKLocalPlayer isAuthenticated]. Returning false (offline mode)!");
        false // Tells the game we are not logged into Game Center!
    }

    - (())authenticateWithCompletionHandler:(id)handler {
        println!("🎮 LOG: Caught [GKLocalPlayer authenticateWithCompletionHandler:]. Absorbing safely!");
        // We just absorb this so it doesn't try to pop up a login screen!
    }
    
// TODO
@end

};

pub const GKPlayerAuthenticationDidChangeNotificationName: &str =
    "GKPlayerAuthenticationDidChangeNotificationName";

/// `NSNotificationName` values.
pub const CONSTANTS: ConstantExports = &[(
    "_GKPlayerAuthenticationDidChangeNotificationName",
    HostConstant::NSString(GKPlayerAuthenticationDidChangeNotificationName),
)];
