/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Core Motion framework.

use crate::dyld::HostDylib;
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreMotion.framework/CoreMotion",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};

struct CMMotionManagerHostObject;
impl HostObject for CMMotionManagerHostObject {}

struct CMAccelerometerDataHostObject;
impl HostObject for CMAccelerometerDataHostObject {}

const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation CMMotionManager: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CMMotionManagerHostObject), &mut env.mem)
}
- (id)init { this }

- (bool)isGyroAvailable { false }
- (bool)isDeviceMotionAvailable { false }
- (bool)isAccelerometerAvailable { true }

- (())setAccelerometerUpdateInterval:(f64)_interval {}
- (())startAccelerometerUpdates {}
- (())setGyroUpdateInterval:(f64)_interval {}
- (())startGyroUpdates {}
- (())setDeviceMotionUpdateInterval:(f64)_interval {}
- (())startDeviceMotionUpdates {}

- (bool)isDeviceMotionActive { false }
- (bool)isAccelerometerActive { true }
- (bool)isGyroActive { false }

- (id)deviceMotion { nil }
- (id)gyroData { nil }

- (id)accelerometerData {
    let data: id = msg_class![env; CMAccelerometerData alloc];
    msg![env; data init]
}

@end

@implementation CMAccelerometerData: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CMAccelerometerDataHostObject), &mut env.mem)
}
- (id)init { this }

// 🏎️ THE ULTIMATE STRET HACK: Bypass the compiler and write directly to RAM!
- (())acceleration {
    // 1. Intercept the hidden memory pointer from the shifted 'this' register
    let stret_ptr = this.to_bits();
    
    // 2. Grab the physical Android hardware sensor data safely
    let options = env.options.clone();
    let (x, y, z) = env.window.as_ref().unwrap().get_acceleration(&options);
    
    // 3. Create raw memory pointers to Asphalt 8's struct buffer
    let ptr_x: crate::mem::MutPtr<f64> = crate::mem::Ptr::from_bits(stret_ptr);
    let ptr_y: crate::mem::MutPtr<f64> = crate::mem::Ptr::from_bits(stret_ptr + 8);
    let ptr_z: crate::mem::MutPtr<f64> = crate::mem::Ptr::from_bits(stret_ptr + 16);
    
    // 4. Forcefully write the 3 doubles (f64) directly into the guest's RAM
    env.mem.write(ptr_x, x as f64);
    env.mem.write(ptr_y, y as f64);
    env.mem.write(ptr_z, z as f64);
    
    // 5. Return void to satisfy the compiler; the struct is perfectly constructed in memory!
}

@end

};
