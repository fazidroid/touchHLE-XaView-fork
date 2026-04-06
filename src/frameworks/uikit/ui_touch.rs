/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UITouch`.

use super::ui_event;
use crate::frameworks::core_graphics::{CGPoint, CGRect};
use crate::frameworks::foundation::{NSInteger, NSTimeInterval, NSUInteger};
use crate::mem::MutVoidPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::window::{Coords, Event, FingerId};
use crate::Environment;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::HashSet;

pub type UITouchPhase = NSInteger;
pub const UITouchPhaseBegan: UITouchPhase = 0;
pub const UITouchPhaseMoved: UITouchPhase = 1;
pub const UITouchPhaseStationary: UITouchPhase = 2;
pub const UITouchPhaseEnded: UITouchPhase = 3;

#[derive(Default)]
pub struct State {
    current_touches: HashMap<FingerId, id>,
}

pub(super) struct UITouchHostObject {
    /// Strong reference to the `UIView`
    pub(super) view: id,
    /// Strong reference to the `UIWindow`, used as a reference for co-ordinate
    /// space conversion
    pub(super) window: id,
    /// Relative to the screen
    location: CGPoint,
    /// Relative to the screen
    previous_location: CGPoint,
    timestamp: NSTimeInterval,
    phase: UITouchPhase,
}
impl HostObject for UITouchHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITouch: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UITouchHostObject {
        view: nil,
        window: nil,
        location: CGPoint { x: 0.0, y: 0.0 },
        previous_location: CGPoint { x: 0.0, y: 0.0 },
        timestamp: 0.0,
        phase: UITouchPhaseBegan,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())dealloc {
    let &mut UITouchHostObject { view, window, .. } = env.objc.borrow_mut(this);
    release(env, view);
    release(env, window);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (CGPoint)locationInView:(id)that_view { // UIView*
    let &UITouchHostObject { location, window, .. } = env.objc.borrow(this);
    let location_in_window: CGPoint = msg![env; window convertPoint:location fromWindow:nil];
    let mut result: CGPoint = if that_view == nil {
        location_in_window
    } else {
        msg![env; that_view convertPoint:location_in_window fromView:window]
    };
    
    // ===== DYNAMIC ANDROID BOUNDS CLAMPING =====
    // Dynamically scales to either Portrait (320x480) or Landscape (480x320) based on the current view!
    if that_view != nil {
        let bounds: CGRect = msg![env; that_view bounds];
        if bounds.size.width > 0.0 && bounds.size.height > 0.0 {
            if result.x < 0.0 { result.x = 0.0; }
            if result.y < 0.0 { result.y = 0.0; }
            // Subtract 1.0 to ensure the touch strictly falls inside C++ array boundaries
            if result.x >= bounds.size.width { result.x = bounds.size.width - 1.0; }
            if result.y >= bounds.size.height { result.y = bounds.size.height - 1.0; }
        }
    }
    result
}

- (CGPoint)previousLocationInView:(id)that_view { // UIView*
    let &UITouchHostObject { previous_location, window, .. } = env.objc.borrow(this);
    let location_in_window: CGPoint = msg![env; window convertPoint:previous_location fromWindow:nil];
    let mut result: CGPoint = if that_view == nil {
        location_in_window
    } else {
        msg![env; that_view convertPoint:location_in_window fromView:window]
    };

    if that_view != nil {
        let bounds: CGRect = msg![env; that_view bounds];
        if bounds.size.width > 0.0 && bounds.size.height > 0.0 {
            if result.x < 0.0 { result.x = 0.0; }
            if result.y < 0.0 { result.y = 0.0; }
            if result.x >= bounds.size.width { result.x = bounds.size.width - 1.0; }
            if result.y >= bounds.size.height { result.y = bounds.size.height - 1.0; }
        }
    }
    result
}

- (id)view {
    env.objc.borrow::<UITouchHostObject>(this).view
}

- (NSTimeInterval)timestamp {
    env.objc.borrow::<UITouchHostObject>(this).timestamp
}

- (NSUInteger)tapCount {
    1 // TODO: support double-taps etc
}

- (UITouchPhase)phase {
    env.objc.borrow::<UITouchHostObject>(this).phase
}

@end

};

/// [super::handle_events] will forward touch events to this function.
pub fn handle_event(env: &mut Environment, event: Event) {
    // before processing anything, we mark all current touches as stationary
    let current_touches = &env.framework_state.uikit.ui_touch.current_touches;
    for &touch in (*current_touches).values() {
        env.objc.borrow_mut::<UITouchHostObject>(touch).phase = UITouchPhaseStationary;
    }
    match event {
        Event::TouchesDown(map) => handle_touches_down(env, map),
        Event::TouchesMove(map) => handle_touches_move(env, map),
        Event::TouchesUp(map) => handle_touches_up(env, map),
        _ => unreachable!(),
    }
}

fn handle_touches_down(env: &mut Environment, map: HashMap<FingerId, Coords>) {
    let pool: id = msg_class![env; NSAutoreleasePool new];

    let timestamp: NSTimeInterval = {
        let process_info = msg_class![env; NSProcessInfo processInfo];
        msg![env; process_info systemUptime]
    };

    let touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
    for (finger_id, coords) in map {
        let current_touches = &mut env.framework_state.uikit.ui_touch.current_touches;
        if current_touches.contains_key(&finger_id) {
            assert_eq!(current_touches.len(), 1);
            log!(
                "Warning: New touch {:?} initiated but current touch did not end yet, treating as movement.",
                finger_id
            );
            return handle_touches_move(env, HashMap::from([(finger_id, coords)]));
        }

        log_dbg!("Finger {:?} touch down: {:?}", finger_id, coords);
        let location = CGPoint {
            x: coords.0,
            y: coords.1,
        };
        let new_touch: id = msg_class![env; UITouch alloc];
        *env.objc.borrow_mut(new_touch) = UITouchHostObject {
            view: nil,
            window: nil,
            location,
            previous_location: location,
            timestamp,
            phase: UITouchPhaseBegan,
        };
        autorelease(env, new_touch);

        let _: () = msg![env; touches addObject:new_touch];

        let _ = &env
            .framework_state
            .uikit
            .ui_touch
            .current_touches
            .insert(finger_id, new_touch);
        retain(env, new_touch);
    }

    let all_touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
    for &touch in env
        .framework_state
        .uikit
        .ui_touch
        .current_touches
        .clone()
        .values()
    {
        let _: () = msg![env; all_touches addObject:touch];
    }

    let event = ui_event::new_event(env, all_touches);
    autorelease(env, event);
    
    let mut view_touches: HashMap<id, id> = HashMap::new();
    let touches_arr: id = msg![env; touches allObjects];
    let touches_count: NSUInteger = msg![env; touches_arr count];
    
    for i in 0..touches_count {
        let touch: id = msg![env; touches_arr objectAtIndex:i];
        let &UITouchHostObject { location, .. } = env.objc.borrow(touch);

        // ===== SMART VIEW TARGETING =====
        let windows = env.framework_state.uikit.ui_view.ui_window.windows.clone();
        
        let mut target_view: id = nil;
        let mut target_window: id = nil;

        // Step 1: Try precise hit testing
        for window in windows.clone().into_iter().rev() {
            let loc: CGPoint = msg![env; window convertPoint:location fromWindow:nil];
            let is_inside: bool = msg![env; window pointInside:loc withEvent:event];
            if is_inside {
                let v: id = msg![env; window hitTest:loc withEvent:event];
                if v != nil {
                    target_view = v;
                    target_window = window;
                    break;
                }
            }
        }

        // Step 2: Fallback for Android bounds mismatches
        if target_view == nil {
            let key_window = env.framework_state.uikit.ui_view.ui_window.key_window;
            let main_window = key_window.unwrap_or_else(|| windows.first().copied().unwrap_or(nil));
            
            if main_window != nil {
                target_window = main_window;
                let subviews: id = msg![env; main_window subviews];
                let count: NSUInteger = msg![env; subviews count];
                
                // Scan layers from top to bottom to find the real interactive EAGLView (ignores dead background images)
                for i in (0..count).rev() {
                    let v: id = msg![env; subviews objectAtIndex:i];
                    let is_hidden: bool = msg![env; v isHidden];
                    let is_interactive: bool = msg![env; v isUserInteractionEnabled];
                    
                    if !is_hidden && is_interactive {
                        target_view = v;
                        log!("Touch bounds bypass: Routing into active game layer {:?}", target_view);
                        break;
                    }
                }
                
                if target_view == nil {
                    target_view = main_window;
                }
            } else {
                log!("Couldn't find any window for touch at {:?}, discarding", location);
                continue;
            }
        }

        let view = target_view;
        let window = target_window;
        // ===============================

        if let Entry::Vacant(e) = view_touches.entry(view) {
            let touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
            e.insert(touches);
        }
        let touches: id = *view_touches.get(&view).unwrap();
        let _: () = msg![env; touches addObject:touch];

        retain(env, view);
        retain(env, window);
        {
            let new_touch = env.objc.borrow_mut::<UITouchHostObject>(touch);
            new_touch.view = view;
            new_touch.window = window;
        }
    }

    for (view, touches) in view_touches {
        log_dbg!(
            "Sending [{:?} touchesBegan:{:?} withEvent:{:?}]",
            view,
            touches,
            event
        );
        let _: () = msg![env; view touchesBegan:touches withEvent:event];
    }

    release(env, pool);
}

fn handle_touches_move(env: &mut Environment, map: HashMap<FingerId, Coords>) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let timestamp: NSTimeInterval = {
        let process_info = msg_class![env; NSProcessInfo processInfo];
        msg![env; process_info systemUptime]
    };

    let touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
    let mut view_touches: HashMap<id, id> = HashMap::new();
    for (finger_id, coords) in map {
        let Some(&touch) = env
            .framework_state
            .uikit
            .ui_touch
            .current_touches
            .get(&finger_id)
        else {
            continue;
        };

        let location = CGPoint {
            x: coords.0,
            y: coords.1,
        };
        let view = env.objc.borrow::<UITouchHostObject>(touch).view;
        let host_object = env.objc.borrow_mut::<UITouchHostObject>(touch);

        if host_object.location == location {
            continue;
        }

        host_object.previous_location = host_object.location;
        host_object.location = location;
        host_object.timestamp = timestamp;
        assert_eq!(host_object.phase, UITouchPhaseStationary);
        host_object.phase = UITouchPhaseMoved;

        let _: () = msg![env; touches addObject:touch];
        if let Entry::Vacant(e) = view_touches.entry(view) {
            let touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
            e.insert(touches);
        }
        let touches: id = *view_touches.get(&view).unwrap();
        let _: () = msg![env; touches addObject:touch];
    }

    let all_touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
    for &touch in env
        .framework_state
        .uikit
        .ui_touch
        .current_touches
        .clone()
        .values()
    {
        let _: () = msg![env; all_touches addObject:touch];
    }

    let event = ui_event::new_event(env, all_touches);
    autorelease(env, event);
    for (view, touches) in view_touches {
        let _: () = msg![env; view touchesMoved:touches withEvent:event];
    }

    release(env, pool);
}

fn handle_touches_up(env: &mut Environment, map: HashMap<FingerId, Coords>) {
    let pool: id = msg_class![env; NSAutoreleasePool new];
    let timestamp: NSTimeInterval = {
        let process_info = msg_class![env; NSProcessInfo processInfo];
        msg![env; process_info systemUptime]
    };

    let touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
    let all_touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
    for &touch in env
        .framework_state
        .uikit
        .ui_touch
        .current_touches
        .clone()
        .values()
    {
        let _: () = msg![env; all_touches addObject:touch];
    }

    let mut view_touches: HashMap<id, id> = HashMap::new();
    for (finger_id, coords) in map {
        let Some(&touch) = env
            .framework_state
            .uikit
            .ui_touch
            .current_touches
            .get(&finger_id)
        else {
            continue;
        };

        let location = CGPoint {
            x: coords.0,
            y: coords.1,
        };
        let view = env.objc.borrow::<UITouchHostObject>(touch).view;
        let host_object = env.objc.borrow_mut::<UITouchHostObject>(touch);
        host_object.previous_location = host_object.location;
        host_object.location = location;
        host_object.timestamp = timestamp;
        assert_eq!(host_object.phase, UITouchPhaseStationary);
        host_object.phase = UITouchPhaseEnded;

        let _: () = msg![env; touches addObject:touch];
        if let Entry::Vacant(e) = view_touches.entry(view) {
            let touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
            e.insert(touches);
        }
        let touches: id = *view_touches.get(&view).unwrap();
        let _: () = msg![env; touches addObject:touch];

        let _ = &env
            .framework_state
            .uikit
            .ui_touch
            .current_touches
            .remove(&finger_id);
        release(env, touch);
    }

    let event = ui_event::new_event(env, all_touches);
    autorelease(env, event);

    for (view, touches) in view_touches {
        let _: () = msg![env; view touchesEnded:touches withEvent:event];
    }

    release(env, pool);
}
