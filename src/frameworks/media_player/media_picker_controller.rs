/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaPickerController`.

use crate::objc::{objc_classes, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaPickerController: UIViewController

- (id)initWithMediaTypes:(i32)mediaTypes {
        println!("🎮 LOG: Caught [MPMediaPickerController initWithMediaTypes:{}]. Returning initialized picker!", mediaTypes);
        // Call the superclass init to properly set up the view controller!
        crate::msg_super![env; this init]
    }

    // 🏎️ PROACTIVE STUBS: Catch the configuration settings!
    - (())setDelegate:(id)delegate {
        println!("🎮 LOG: Caught [MPMediaPickerController setDelegate:]. Absorbing safely!");
    }
    
    - (())setAllowsPickingMultipleItems:(bool)allows { }
    
    - (())setPrompt:(id)prompt { }

// TODO
@end

};
