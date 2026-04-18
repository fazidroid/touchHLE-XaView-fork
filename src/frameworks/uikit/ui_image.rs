/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIImage`.

use crate::frameworks::core_graphics::cg_context::CGContextDrawImage;
use crate::frameworks::core_graphics::cg_image::{
    self, CGImageGetHeight, CGImageGetWidth, CGImageRef, CGImageRelease, CGImageRetain,
};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::{ns_data, ns_string, NSInteger};
use crate::frameworks::uikit::ui_graphics::UIGraphicsGetCurrentContext;
use crate::fs::GuestPath;
use crate::image::Image;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;
use std::collections::HashMap;

const CACHE_SIZE: usize = 10;

#[derive(Default)]
pub struct State {
    cached_images: HashMap<String, id>,
}
impl State {
    fn get(env: &Environment) -> &Self {
        &env.framework_state.uikit.ui_image
    }
    fn get_mut(env: &mut Environment) -> &mut Self {
        &mut env.framework_state.uikit.ui_image
    }
}

struct UIImageHostObject {
    cg_image: CGImageRef,
}
impl HostObject for UIImageHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIImage: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIImageHostObject { cg_image: nil });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)imageWithCGImage:(CGImageRef)cg_image {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCGImage:cg_image];
    autorelease(env, new)
}

+ (id)imageNamed:(id)name {
    let bundle: id = msg_class![env; NSBundle mainBundle];
    let path: id = msg![env; bundle pathForResource:name ofType:nil];
    let name_str = ns_string::to_rust_string(env, name).to_string();
    if path == nil {
        log!("Warning: [UIImage imageNamed:{:?}] => nil", name_str);
        return nil;
    }
    if State::get(env).cached_images.len() > CACHE_SIZE {
        let cache = std::mem::take(&mut State::get_mut(env).cached_images);
        for (_, img) in cache {
            release(env, img);
        }
    }
    if !State::get(env).cached_images.contains_key(&name_str) {
        let img = msg![env; this imageWithContentsOfFile:path];
        retain(env, img);
        State::get_mut(env).cached_images.insert(name_str.clone(), img);
    }
    *State::get(env).cached_images.get(&name_str).unwrap()
}

+ (id)imageWithContentsOfFile:(id)path {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfFile:path];
    autorelease(env, new)
}

+ (id)imageWithData:(id)data {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithData:data];
    autorelease(env, new)
}

- (())dealloc {
    let &UIImageHostObject { cg_image } = env.objc.borrow(this);
    if cg_image != nil {
        CGImageRelease(env, cg_image);
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)initWithCGImage:(CGImageRef)cg_image {
    if cg_image != nil {
        CGImageRetain(env, cg_image);
    }
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)initWithContentsOfFile:(id)path {
    if path == nil {
        return nil;
    }
    let path = ns_string::to_rust_string(env, path);
    let Ok(bytes) = env.fs.read(GuestPath::new(&path)) else {
        release(env, this);
        return nil;
    };
    
    // ==========================================================
    // 🏎️ DYNAMIC IMAGE BYPASS: NOVA 3 Exclusive PVR Absorb
    // ==========================================================
    let mut is_nova3 = false;
    if !env.is_app_picker {
        is_nova3 = env.bundle.bundle_identifier().starts_with("com.gameloft.nova3");
    }

    let image = if is_nova3 {
        match Image::from_bytes(&bytes) {
            Ok(img) => img,
            Err(_) => {
                println!("🎮 LOG: NOVA 3 EXCLUSIVE BYPASS! Safely ignored unknown proprietary image format in file [{}]!", path);
                release(env, this);
                return nil;
            }
        }
    } else {
        // Standard touchHLE behavior for all other games!
        Image::from_bytes(&bytes).unwrap()
    };
    
    let cg_image = cg_image::from_image(env, image);
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)initWithData:(id)data {
    let slice = ns_data::to_rust_slice(env, data);
    
    // ==========================================================
    // 🏎️ DYNAMIC IMAGE BYPASS: NOVA 3 Exclusive PVR Absorb
    // ==========================================================
    let mut is_nova3 = false;
    if !env.is_app_picker {
        is_nova3 = env.bundle.bundle_identifier().starts_with("com.gameloft.nova3");
    }

    let image = if is_nova3 {
        match Image::from_bytes(slice) {
            Ok(img) => img,
            Err(_) => {
                println!("🎮 LOG: NOVA 3 EXCLUSIVE BYPASS! Safely ignored unknown proprietary image format in NSData payload!");
                release(env, this);
                return nil;
            }
        }
    } else {
        // Standard touchHLE behavior for all other games!
        Image::from_bytes(slice).unwrap()
    };
    
    let cg_image = cg_image::from_image(env, image);
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)stretchableImageWithLeftCapWidth:(NSInteger)_leftCapWidth
                          topCapHeight:(NSInteger)_topCapHeight {
    retain(env, this)
}

- (CGImageRef)CGImage {
    env.objc.borrow::<UIImageHostObject>(this).cg_image
}

- (NSInteger)imageOrientation {
    0
}

- (CGSize)size {
    let image = env.objc.borrow::<UIImageHostObject>(this).cg_image;
    if image == nil {
        return CGSize { width: 0.0, height: 0.0 };
    }
    let (width, height) = cg_image::borrow_image(&env.objc, image).dimensions();
    CGSize {
        width: width as _,
        height: height as _,
    }
}

- (CGFloat)scale {
    1.0
}

- (())drawInRect:(CGRect)rect {
    let context = UIGraphicsGetCurrentContext(env);
    let image = env.objc.borrow::<UIImageHostObject>(this).cg_image;
    if image != nil && context != nil {
        CGContextDrawImage(env, context, rect, image);
    }
}

- (())drawAtPoint:(CGPoint)point {
    let context = UIGraphicsGetCurrentContext(env);
    let image = env.objc.borrow::<UIImageHostObject>(this).cg_image;
    if image != nil && context != nil {
        let rect = CGRect {
            origin: point,
            size: CGSize {
                width: CGImageGetWidth(env, image) as CGFloat,
                height: CGImageGetHeight(env, image) as CGFloat,
            }
        };
        CGContextDrawImage(env, context, rect, image);
    }
}

@end

@implementation UIImageNibPlaceholder: UIImage

- (id)initWithCoder:(id)coder {
    release(env, this);
    let key_ns_string = get_static_str(env, "UIResourceName");
    let resource_name: id = msg![env; coder decodeObjectForKey:key_ns_string];
    let res = msg_class![env; UIImage imageNamed:resource_name];
    retain(env, res)
}

@end

};
