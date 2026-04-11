/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Core Motion framework.

use crate::dyld::HostDylib;
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr};
use crate::Environment;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreMotion.framework/CoreMotion",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};

// 🏎️ Dummy objects to hold our classes in memory
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

// Disable advanced gyro/motion to prevent engine crashes, enable pure accelerometer
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
    // 🏎️ Create the data packet and hand it to Asphalt 8
    let data: id = msg_class![env; CMAccelerometerData alloc];
    msg![env; data init]
}

@end

@implementation CMAccelerometerData: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CMAccelerometerDataHostObject), &mut env.mem)
}
- (id)init { this }

- ((f64, f64, f64))acceleration {
    // 🏎️ Grab the physical Android hardware sensor data!
    let options = env.options.clone();
    let (x, y, z) = env.window.get_acceleration(&options);
    
    // Pass it back to the game engine as a 3D tuple
    (x as f64, y as f64, z as f64)
}

@end

};
