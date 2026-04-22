/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIScreen`.

use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::objc::{autorelease, id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr};

#[derive(Default)]
pub struct State {
    main_screen: Option<id>,
}

// Host object for UIScreenMode
struct UIScreenModeHostObject {
    size: CGSize,
    pixel_aspect_ratio: CGFloat,
}
impl HostObject for UIScreenModeHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// For now this is a singleton (the only instance is returned by mainScreen),
// so there are hardcoded assumptions related to that.
@implementation UIScreen: NSObject

+ (id)mainScreen {
    if let Some(screen) = env.framework_state.uikit.ui_screen.main_screen {
        screen
    } else {
        let new = env.objc.alloc_static_object(
            this,
            Box::new(TrivialHostObject),
            &mut env.mem
        );
        env.framework_state.uikit.ui_screen.main_screen = Some(new);
        new
   }
}

+ (id)screens {
    // ReturnMainScreenArray
    let main_screen: id = msg_class![env; UIScreen mainScreen];
    let arr = crate::frameworks::foundation::ns_array::from_vec(env, vec![main_screen]);
    crate::objc::autorelease(env, arr)
}

- (id)retain { this }
- (())release {}
- (id)autorelease { this }

// TODO: more accessors

- (CGRect)bounds {
    // While Apple's documentation says this changes with the interface
    // orientation, https://useyourloaf.com/blog/uiscreen-bounds-in-ios-8/ says
    // ths wasn't the case prior to iOS 8.
    let (width, height) = env.window().device_family().portrait_size();
    CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width: width as f32, height: height as f32 },
    }
}

- (CGRect)applicationFrame {
    // FIXME: Does this change depending on the status bar orientation?
    let mut bounds: CGRect = msg![env; this bounds];
    const STATUS_BAR_HEIGHT: f32 = 20.0;
    if !env.framework_state.uikit.ui_application.status_bar_hidden {
        bounds.origin.y += STATUS_BAR_HEIGHT;
        bounds.size.height -= STATUS_BAR_HEIGHT;
    }
    bounds
}

- (CGFloat)scale {
    // SupportRetinaScale
    let model = env.options.device_model.as_deref().unwrap_or("");
    let is_retina = model.starts_with("iPhone3")
        || model.starts_with("iPhone4")
        || model.starts_with("iPhone5")
        || model.starts_with("iPod4")
        || model.starts_with("iPod5")
        || model.starts_with("iPad3")
        || model.starts_with("iPad4");

    if is_retina {
        2.0
    } else {
        1.0
    }
}

- (id)displayLinkWithTarget:(id)target selector:(id)sel {
    // ReturnDisplayLinkStub
    let cls = env.objc.get_known_class("CADisplayLink", &mut env.mem);
    msg![env; cls displayLinkWithTarget:target selector:sel]
}

- (id)currentMode {
    log!("UIScreen currentMode stub called");
    let bounds: CGRect = msg![env; this bounds];
    let mode: id = msg_class![env; UIScreenMode alloc];
    let mode: id = msg![env; mode initWithSize:bounds.size pixelAspectRatio:1.0f];
    autorelease(env, mode)
}

@end

// UIScreenMode stub implementation
@implementation UIScreenMode: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIScreenModeHostObject {
        size: CGSize { width: 0.0, height: 0.0 },
        pixel_aspect_ratio: 1.0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithSize:(CGSize)size pixelAspectRatio:(CGFloat)ratio {
    let host = env.objc.borrow_mut::<UIScreenModeHostObject>(this);
    host.size = size;
    host.pixel_aspect_ratio = ratio;
    this
}

- (CGSize)size {
    env.objc.borrow::<UIScreenModeHostObject>(this).size
}

- (CGFloat)pixelAspectRatio {
    env.objc.borrow::<UIScreenModeHostObject>(this).pixel_aspect_ratio
}

@end

};
