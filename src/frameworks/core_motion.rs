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

struct CMMotionManagerHostObject {
    accelerometer_handler: Option<id>, // block (CMAccelerometerData → void)
    accelerometer_queue: Option<id>,
    update_interval: f64,
    timer: Option<id>, // NSTimer
}
impl HostObject for CMMotionManagerHostObject {}

struct CMAccelerometerDataHostObject {
    x: f64,
    y: f64,
    z: f64,
}
impl HostObject for CMAccelerometerDataHostObject {}

const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation CMMotionManager: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(CMMotionManagerHostObject {
        accelerometer_handler: None,
        accelerometer_queue: None,
        update_interval: 0.1,
        timer: None,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    this
}

- (bool)isGyroAvailable { false }
- (bool)isDeviceMotionAvailable { false }
- (bool)isAccelerometerAvailable { true }

- (())setAccelerometerUpdateInterval:(f64)interval {
    let host = env.objc.borrow_mut::<CMMotionManagerHostObject>(this);
    host.update_interval = interval;
}

- (())startAccelerometerUpdatesToQueue:(id)queue withHandler:(id)handler {
    log!("CMMotionManager startAccelerometerUpdatesToQueue:withHandler:");

    // Extract values we need while holding the mutable borrow, then drop it.
    let (interval, sel) = {
        let host = env.objc.borrow_mut::<CMMotionManagerHostObject>(this);
        host.accelerometer_handler = Some(handler);
        host.accelerometer_queue = Some(queue);
        let iv = host.update_interval;
        drop(host);
        let sel = env.objc.lookup_selector("_touchHLE_accelTimerFired:").unwrap();
        (iv, sel)
    };

    // Now we can create the timer without the mutable borrow.
    let timer: id = msg_class![env; NSTimer scheduledTimerWithTimeInterval:interval
                                                                    target:this
                                                                  selector:sel
                                                                  userInfo:nil
                                                                   repeats:true];
    // Re-borrow to store the timer.
    env.objc.borrow_mut::<CMMotionManagerHostObject>(this).timer = Some(timer);
}

- (())startAccelerometerUpdates {
    log!("CMMotionManager startAccelerometerUpdates stub called");
}

- (())setDeviceMotionUpdateInterval:(f64)interval {
    log!("CMMotionManager setDeviceMotionUpdateInterval: {} stub called", interval);
}

- (())setGyroUpdateInterval:(f64)interval {
    log!("CMMotionManager setGyroUpdateInterval: {} stub called", interval);
}

- (())stopAccelerometerUpdates {
    // Take the timer and clear the handler/queue while holding the mutable borrow.
    let timer = {
        let host = env.objc.borrow_mut::<CMMotionManagerHostObject>(this);
        host.accelerometer_handler = None;
        host.accelerometer_queue = None;
        host.timer.take()
    };

    // Invalidate the timer without holding the mutable borrow.
    if let Some(t) = timer {
        let _: () = msg![env; t invalidate];
    }
}

// Internal timer callback – reads UIAccelerometer and calls the handler block.
- (())_touchHLE_accelTimerFired:(id)_timer {
    // The game will obtain acceleration data by polling `accelerometerData`.
    // No need to call the handler here.
    log_dbg!("CMMotionManager: timer fired");
}

- (bool)isDeviceMotionActive { false }
- (bool)isAccelerometerActive { true }
- (bool)isGyroActive { false }

- (id)deviceMotion { nil }
- (id)gyroData { nil }

- (id)accelerometerData {
    let accel: id = msg_class![env; UIAccelerometer sharedAccelerometer];
    let acceleration: id = msg![env; accel acceleration];
    let x: f64 = msg![env; acceleration x];
    let y: f64 = msg![env; acceleration y];
    let z: f64 = msg![env; acceleration z];
    let data: id = msg_class![env; CMAccelerometerData alloc];
    msg![env; data initWithX:x y:y z:z]
}

@end

@implementation CMAccelerometerData: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(CMAccelerometerDataHostObject {
        x: 0.0, y: 0.0, z: -1.0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithX:(f64)x y:(f64)y z:(f64)z {
    let host = env.objc.borrow_mut::<CMAccelerometerDataHostObject>(this);
    host.x = x;
    host.y = y;
    host.z = z;
    this
}

- (())acceleration {
    // Used by older games (Asphalt 8) that expect a “stret” return.
    let host = env.objc.borrow::<CMAccelerometerDataHostObject>(this);
    let stret_ptr = this.to_bits();
    let ptr_x: crate::mem::MutPtr<f64> = crate::mem::Ptr::from_bits(stret_ptr);
    let ptr_y: crate::mem::MutPtr<f64> = crate::mem::Ptr::from_bits(stret_ptr + 8);
    let ptr_z: crate::mem::MutPtr<f64> = crate::mem::Ptr::from_bits(stret_ptr + 16);
    env.mem.write(ptr_x, host.x);
    env.mem.write(ptr_y, host.y);
    env.mem.write(ptr_z, host.z);
}

// Proper getters for newer code that accesses .acceleration.x etc.
- (f64)x {
    env.objc.borrow::<CMAccelerometerDataHostObject>(this).x
}
- (f64)y {
    env.objc.borrow::<CMAccelerometerDataHostObject>(this).y
}
- (f64)z {
    env.objc.borrow::<CMAccelerometerDataHostObject>(this).z
}

@end

};
