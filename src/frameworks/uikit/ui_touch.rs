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
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, Class, ClassExports, HostObject,
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
    pub(super) view: id,
    pub(super) window: id,
    location: CGPoint,
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

- (CGPoint)locationInView:(id)that_view { 
    let &UITouchHostObject { location, window, .. } = env.objc.borrow(this);
    let location_in_window: CGPoint = msg![env; window convertPoint:location fromWindow:nil];
    let mut result: CGPoint = if that_view == nil {
        location_in_window
    } else {
        msg![env; that_view convertPoint:location_in_window fromView:window]
    };
    
    if that_view != nil {
        let bounds: CGRect = msg![env; that_view bounds];
        let w = bounds.size.width;
        let h = bounds.size.height;
        if w > 0.0 && h > 0.0 {
            if result.x < 0.0 { result.x = 0.0; }
            if result.y < 0.0 { result.y = 0.0; }
            if result.x >= w { result.x = w - 1.0; }
            if result.y >= h { result.y = h - 1.0; }
        }
    }
    result
}

- (CGPoint)previousLocationInView:(id)that_view {
    let &UITouchHostObject { previous_location, window, .. } = env.objc.borrow(this);
    let location_in_window: CGPoint = msg![env; window convertPoint:previous_location fromWindow:nil];
    let mut result: CGPoint = if that_view == nil {
        location_in_window
    } else {
        msg![env; that_view convertPoint:location_in_window fromView:window]
    };

    if that_view != nil {
        let bounds: CGRect = msg![env; that_view bounds];
        let w = bounds.size.width;
        let h = bounds.size.height;
        if w > 0.0 && h > 0.0 {
            if result.x < 0.0 { result.x = 0.0; }
            if result.y < 0.0 { result.y = 0.0; }
            if result.x >= w { result.x = w - 1.0; }
            if result.y >= h { result.y = h - 1.0; }
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
    1 
}

- (UITouchPhase)phase {
    env.objc.borrow::<UITouchHostObject>(this).phase
}

@end

};

pub fn handle_event(env: &mut Environment, event: Event) {
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
            return handle_touches_move(env, HashMap::from([(finger_id, coords)]));
        }

        let location = CGPoint { x: coords.0, y: coords.1 };
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
    for &touch in env.framework_state.uikit.ui_touch.current_touches.clone().values() {
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

        let windows = env.framework_state.uikit.ui_view.ui_window.windows.clone();
        
        let mut target_view: id = nil;
        let mut target_window: id = nil;

        for window in windows.into_iter().rev() {
            let mut loc_in_win: CGPoint = msg![env; window convertPoint:location fromWindow:nil];
            
            // 1. DYNAMIC CLAMPER
            let bounds: CGRect = msg![env; window bounds];
            let w = bounds.size.width;
            let h = bounds.size.height;
            if w > 0.0 && h > 0.0 {
                if loc_in_win.x < 0.0 { loc_in_win.x = 0.0; }
                if loc_in_win.y < 0.0 { loc_in_win.y = 0.0; }
                if loc_in_win.x >= w { loc_in_win.x = w - 1.0; }
                if loc_in_win.y >= h { loc_in_win.y = h - 1.0; }
            }
            
            // 2. NATIVE HIT TEST
            let mut hit_view: id = msg![env; window hitTest:loc_in_win withEvent:event];
            
            let uiview_class = env.objc.get_known_class("UIView", &mut env.mem);
            let is_exact_uiview: bool = if hit_view != nil { msg![env; hit_view isMemberOfClass:uiview_class] } else { false };
            let uiimageview_class = env.objc.get_known_class("UIImageView", &mut env.mem);
            let is_imageview: bool = if hit_view != nil { msg![env; hit_view isKindOfClass:uiimageview_class] } else { false };

            // 3. SHIELD PIERCER
            // If the hit view is a generic emulator UIView (like an invisible error overlay) or image, pierce through it!
            if hit_view == nil || is_exact_uiview || is_imageview {
                let subviews: id = msg![env; window subviews];
                let count: NSUInteger = msg![env; subviews count];
                
                let mut found_custom = false;
                for j in (0..count).rev() {
                    let v: id = msg![env; subviews objectAtIndex:j];
                    let is_generic: bool = msg![env; v isMemberOfClass:uiview_class];
                    let is_img: bool = msg![env; v isKindOfClass:uiimageview_class];
                    let is_hidden: bool = msg![env; v isHidden];
                    let is_interactive: bool = msg![env; v isUserInteractionEnabled];
                    
                    // Route exclusively to the custom interactive layer (the 3D game engine)
                    if !is_generic && !is_img && !is_hidden && is_interactive {
                        hit_view = v;
                        found_custom = true;
                        log!("Shield Pierced! Rerouted touch directly to 3D Game Engine {:?}", hit_view);
                        break;
                    }
                }
                
                if !found_custom && hit_view == nil {
                    hit_view = window; // Ultimate fallback
                }
            }

            if hit_view != nil {
                target_view = hit_view;
                target_window = window;
                break;
            }
        }

        if target_view == nil {
            continue;
        }

        if let Entry::Vacant(e) = view_touches.entry(target_view) {
            let touches: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];
            e.insert(touches);
        }
        let touches: id = *view_touches.get(&target_view).unwrap();
        let _: () = msg![env; touches addObject:touch];

        retain(env, target_view);
        retain(env, target_window);
        {
            let new_touch = env.objc.borrow_mut::<UITouchHostObject>(touch);
            new_touch.view = target_view;
            new_touch.window = target_window;
        }
    }

    for (view, touches) in view_touches {
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
        let Some(&touch) = env.framework_state.uikit.ui_touch.current_touches.get(&finger_id) else {
            continue;
        };

        let location = CGPoint { x: coords.0, y: coords.1 };
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
    for &touch in env.framework_state.uikit.ui_touch.current_touches.clone().values() {
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
    for &touch in env.framework_state.uikit.ui_touch.current_touches.clone().values() {
        let _: () = msg![env; all_touches addObject:touch];
    }

    let mut view_touches: HashMap<id, id> = HashMap::new();
    for (finger_id, coords) in map {
        let Some(&touch) = env.framework_state.uikit.ui_touch.current_touches.get(&finger_id) else {
            continue;
        };

        let location = CGPoint { x: coords.0, y: coords.1 };
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

        let _ = &env.framework_state.uikit.ui_touch.current_touches.remove(&finger_id);
        release(env, touch);
    }

    let event = ui_event::new_event(env, all_touches);
    autorelease(env, event);

    for (view, touches) in view_touches {
        let _: () = msg![env; view touchesEnded:touches withEvent:event];
    }

    release(env, pool);
}
