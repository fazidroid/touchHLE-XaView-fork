/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CADisplayLink`

use crate::frameworks::foundation::ns_run_loop::NSRunLoopMode;
use crate::frameworks::foundation::ns_timer::set_time_interval;
use crate::frameworks::foundation::NSInteger;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr, SEL,
};

#[derive(Default)]
struct CADisplayLinkHostObject {
    ns_timer: id,
}
impl HostObject for CADisplayLinkHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation CADisplayLink: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CADisplayLinkHostObject::default()), &mut env.mem)
}

+ (id)displayLinkWithTarget:(id)target selector:(SEL)sel {
    let ns_timer = msg_class![env; NSTimer timerWithTimeInterval:(1.0/60.0)
                     target:target
                   selector:sel
                   userInfo:nil
                    repeats:true];
    retain(env, ns_timer);
    let display_link: id = msg![env; this new];
    let host_object = env.objc.borrow_mut::<CADisplayLinkHostObject>(display_link);
    host_object.ns_timer = ns_timer;
    log_dbg!("[CADisplayLink displayLinkWithTarget:{:?} selector:{}] => {:?}", target, sel.as_str(&env.mem), display_link);
    autorelease(env, display_link)
}

- (())setFrameInterval:(NSInteger)frameInterval {
    log_dbg!("[(CADisplayLink*){:?} setFrameInterval:{}]", this, frameInterval);
    
    // ==========================================================
    // 🏎️ GAMELOFT BYPASS: Safely clamp invalid frame intervals!
    // ==========================================================
    let safe_interval = if frameInterval < 1 {
        println!("🎮 LOG: Bypassed CADisplayLink panic! Clamped invalid frameInterval {} to 1.", frameInterval);
        1
    } else {
        frameInterval
    };

    let interval = safe_interval as f64 / 60.0;
    let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
    set_time_interval(env, ns_timer, interval);
}

- (())addToRunLoop:(id)run_loop forMode:(NSRunLoopMode)mode {
    log_dbg!("[(CADisplayLink*){:?} addToRunLoop:{:?} forMode:{:?}]", this, run_loop, mode);
    let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
    () = msg![env; run_loop addTimer:ns_timer forMode:mode];
}

- (())invalidate {
    log_dbg!("[(CADisplayLink*){:?} invalidate]", this);
    let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
    () = msg![env; ns_timer invalidate];
}

- (())dealloc {
    let host_object = env.objc.borrow::<CADisplayLinkHostObject>(this);
    release(env, host_object.ns_timer);
    env.objc.dealloc_object(this, &mut env.mem);
}

// ==========================================================
    // PauseDisplayLink: actually invalidate the underlying NSTimer when paused.
    // Previously this was a no-op which caused the display link to keep firing
    // during movie player sequences, producing extra render frames and flicker.
    // The game re-adds the display link to the run loop (addToRunLoop:forMode:)
    // when it wants to resume, which creates a new timer — so we only need to
    // handle the pause direction here.
    - (())setPaused:(bool)paused {
        log_dbg!("[(CADisplayLink*){:?} setPaused:{}]", this, paused);
        if paused {
            let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
            let is_valid: bool = msg![env; ns_timer isValid];
            if is_valid {
                let _: () = msg![env; ns_timer invalidate];
            }
        }
        // Resuming: the game calls addToRunLoop:forMode: to restart, handled above.
    }

    - (bool)isPaused {
        let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
        let is_valid: bool = msg![env; ns_timer isValid];
        !is_valid
    }

@end

};
