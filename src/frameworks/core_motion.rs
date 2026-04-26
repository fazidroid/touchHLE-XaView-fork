/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Core Motion framework.

use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::HostDylib;
use crate::mem::{ConstPtr, MutPtr, Ptr, SafeRead};
use crate::objc::{id, msg, msg_class, nil, objc_classes, retain, release, ClassExports, HostObject, NSZonePtr};
use crate::Environment;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreMotion.framework/CoreMotion",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};

// ── Host objects ────────────────────────────────────────────────────────────

struct CMMotionManagerHostObject {
    /// Stored accelerometer handler block (retained).
    accel_handler: id,
    /// Stored device-motion handler block (retained).
    device_motion_handler: id,
    /// Accelerometer update interval in seconds.
    accel_interval: f64,
    /// Device motion update interval in seconds.
    motion_interval: f64,
    /// Whether accelerometer updates are running.
    accel_active: bool,
    /// Whether device-motion updates are running.
    motion_active: bool,
}
impl HostObject for CMMotionManagerHostObject {}
impl Default for CMMotionManagerHostObject {
    fn default() -> Self {
        Self {
            accel_handler: nil,
            device_motion_handler: nil,
            accel_interval: 1.0 / 60.0,
            motion_interval: 1.0 / 60.0,
            accel_active: false,
            motion_active: false,
        }
    }
}

struct CMAccelerometerDataHostObject {
    x: f64,
    y: f64,
    z: f64,
}
impl HostObject for CMAccelerometerDataHostObject {}

struct CMDeviceMotionHostObject {
    grav_x: f64,
    grav_y: f64,
    grav_z: f64,
    ua_x: f64,
    ua_y: f64,
    ua_z: f64,
    roll: f64,
    pitch: f64,
    yaw: f64,
}
impl HostObject for CMDeviceMotionHostObject {}

struct CMAttitudeHostObject {
    roll: f64,
    pitch: f64,
    yaw: f64,
}
impl HostObject for CMAttitudeHostObject {}

// ── Block calling helper ─────────────────────────────────────────────────────

/// Call an Objective-C block that takes `(id data, id error) -> ()`.
/// The block's invoke pointer lives at offset 12 in the block struct.
unsafe fn call_block_with_data_and_error(env: &mut Environment, block: id, data: id, error: id) {
    if block == nil {
        return;
    }
    // ObjC block layout: [isa(4), flags(4), reserved(4), invoke(4), ...]
    let invoke_addr: u32 = env.mem.read(Ptr::from_bits(block.to_bits() + 12));
    if invoke_addr == 0 {
        return;
    }
    let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_addr);
    invoke.call_from_host(env, (block, data, error));
}

// ── Accelerometer data helper ────────────────────────────────────────────────

fn make_accel_data(env: &mut Environment) -> id {
    let options = env.options.clone();
    let (x, y, z) = env.window.as_ref().unwrap().get_acceleration(&options);
    let data: id = msg_class![env; CMAccelerometerData alloc];
    let data: id = msg![env; data init];
    *env.objc.borrow_mut::<CMAccelerometerDataHostObject>(data) = CMAccelerometerDataHostObject {
        x: x as f64,
        y: y as f64,
        z: z as f64,
    };
    data
}

// ── Classes ──────────────────────────────────────────────────────────────────

const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// ── CMMotionManager ──────────────────────────────────────────────────────────

@implementation CMMotionManager: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CMMotionManagerHostObject::default()), &mut env.mem)
}
- (id)init { this }

- (())dealloc {
    let obj = env.objc.borrow::<CMMotionManagerHostObject>(this);
    let ah = obj.accel_handler;
    let dh = obj.device_motion_handler;
    if ah != nil { release(env, ah); }
    if dh != nil { release(env, dh); }
}

// Availability — we have a real accelerometer, fake gyro/device-motion off
// unless the game specifically needs them (they'll get nil data).
- (bool)isAccelerometerAvailable { true }
- (bool)isGyroAvailable          { false }
- (bool)isDeviceMotionAvailable  { false }

- (bool)isAccelerometerActive {
    env.objc.borrow::<CMMotionManagerHostObject>(this).accel_active
}
- (bool)isDeviceMotionActive {
    env.objc.borrow::<CMMotionManagerHostObject>(this).motion_active
}
- (bool)isGyroActive { false }

// ── Intervals ────────────────────────────────────────────────────────────────

- (())setAccelerometerUpdateInterval:(f64)interval {
    env.objc.borrow_mut::<CMMotionManagerHostObject>(this).accel_interval = interval;
}
- (())setDeviceMotionUpdateInterval:(f64)interval {
    env.objc.borrow_mut::<CMMotionManagerHostObject>(this).motion_interval = interval;
}
- (())setGyroUpdateInterval:(f64)_interval {}

// ── Polling API ──────────────────────────────────────────────────────────────

- (id)accelerometerData {
    make_accel_data(env)
}

- (id)deviceMotion {
    // Return a fake device-motion object with gravity == raw acceleration.
    let options = env.options.clone();
    let (x, y, z) = env.window.as_ref().unwrap().get_acceleration(&options);
    let dm: id = msg_class![env; CMDeviceMotion alloc];
    let dm: id = msg![env; dm init];
    *env.objc.borrow_mut::<CMDeviceMotionHostObject>(dm) = CMDeviceMotionHostObject {
        grav_x: x as f64,
        grav_y: y as f64,
        grav_z: z as f64,
        ua_x: 0.0, ua_y: 0.0, ua_z: 0.0,
        roll: 0.0, pitch: (x as f64).asin(), yaw: 0.0,
    };
    dm
}

- (id)gyroData { nil }

// ── Push API (handler blocks) ─────────────────────────────────────────────────

- (())startAccelerometerUpdates { 
    env.objc.borrow_mut::<CMMotionManagerHostObject>(this).accel_active = true;
}
- (())stopAccelerometerUpdates {
    env.objc.borrow_mut::<CMMotionManagerHostObject>(this).accel_active = false;
}
- (())startGyroUpdates {}
- (())stopGyroUpdates {}
- (())startDeviceMotionUpdates {
    env.objc.borrow_mut::<CMMotionManagerHostObject>(this).motion_active = true;
}
- (())stopDeviceMotionUpdates {
    env.objc.borrow_mut::<CMMotionManagerHostObject>(this).motion_active = false;
}

- (())startAccelerometerUpdatesToQueue:(id)_queue withHandler:(id)handler {
    log_dbg!("CMMotionManager startAccelerometerUpdatesToQueue:withHandler: storing handler");
    let old = env.objc.borrow::<CMMotionManagerHostObject>(this).accel_handler;
    if old != nil { release(env, old); }
    if handler != nil { retain(env, handler); }
    {
        let obj = env.objc.borrow_mut::<CMMotionManagerHostObject>(this);
        obj.accel_handler = handler;
        obj.accel_active = true;
    }
    // Fire once immediately so the game gets data right away.
    let data = make_accel_data(env);
    if handler != nil {
        unsafe { call_block_with_data_and_error(env, handler, data, nil); }
    }
    release(env, data);
}

- (())startDeviceMotionUpdatesToQueue:(id)_queue withHandler:(id)handler {
    log_dbg!("CMMotionManager startDeviceMotionUpdatesToQueue:withHandler: storing handler");
    let old = env.objc.borrow::<CMMotionManagerHostObject>(this).device_motion_handler;
    if old != nil { release(env, old); }
    if handler != nil { retain(env, handler); }
    {
        let obj = env.objc.borrow_mut::<CMMotionManagerHostObject>(this);
        obj.device_motion_handler = handler;
        obj.motion_active = true;
    }
    // Fire once immediately.
    let dm: id = msg![env; this deviceMotion];
    if handler != nil {
        unsafe { call_block_with_data_and_error(env, handler, dm, nil); }
    }
    release(env, dm);
}

- (())startDeviceMotionUpdatesUsingReferenceFrame:(u32)_frame
                                          toQueue:(id)queue
                                      withHandler:(id)handler {
    msg![env; this startDeviceMotionUpdatesToQueue:queue withHandler:handler]
}

/// Called each frame from the run loop to push fresh sensor data to any
/// registered handler blocks.  Host code can call this from the main loop.
- (())_touchHLE_fireSensorHandlers {
    let obj = env.objc.borrow::<CMMotionManagerHostObject>(this);
    let ah = obj.accel_handler;
    let dh = obj.device_motion_handler;
    let accel_active = obj.accel_active;
    let motion_active = obj.motion_active;
    drop(obj);

    if accel_active && ah != nil {
        let data = make_accel_data(env);
        unsafe { call_block_with_data_and_error(env, ah, data, nil); }
        release(env, data);
    }
    if motion_active && dh != nil {
        let dm: id = msg![env; this deviceMotion];
        unsafe { call_block_with_data_and_error(env, dh, dm, nil); }
        release(env, dm);
    }
}

@end

// ── CMAccelerometerData ───────────────────────────────────────────────────────

@implementation CMAccelerometerData: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CMAccelerometerDataHostObject { x: 0.0, y: 0.0, z: 0.0 }), &mut env.mem)
}
- (id)init { this }

// The `acceleration` property returns a CMAcceleration struct (3× f64).
// It uses the struct-return (stret) convention on ARMv7: the caller passes
// a hidden pointer in r0 before `self` and `_cmd`.  We write the three
// doubles directly into that buffer.
- (())acceleration {
    let stret_ptr = this.to_bits();
    let obj = env.objc.borrow::<CMAccelerometerDataHostObject>(this);
    let (x, y, z) = (obj.x, obj.y, obj.z);
    drop(obj);
    env.mem.write(Ptr::from_bits(stret_ptr),      x);
    env.mem.write(Ptr::from_bits(stret_ptr + 8),  y);
    env.mem.write(Ptr::from_bits(stret_ptr + 16), z);
}

@end

// ── CMDeviceMotion ────────────────────────────────────────────────────────────

@implementation CMDeviceMotion: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CMDeviceMotionHostObject {
        grav_x: 0.0, grav_y: 0.0, grav_z: -1.0,
        ua_x: 0.0, ua_y: 0.0, ua_z: 0.0,
        roll: 0.0, pitch: 0.0, yaw: 0.0,
    }), &mut env.mem)
}
- (id)init { this }

// gravity (stret: 3× f64)
- (())gravity {
    let stret_ptr = this.to_bits();
    let obj = env.objc.borrow::<CMDeviceMotionHostObject>(this);
    let (x, y, z) = (obj.grav_x, obj.grav_y, obj.grav_z);
    drop(obj);
    env.mem.write(Ptr::from_bits(stret_ptr),      x);
    env.mem.write(Ptr::from_bits(stret_ptr + 8),  y);
    env.mem.write(Ptr::from_bits(stret_ptr + 16), z);
}

// userAcceleration (stret: 3× f64)
- (())userAcceleration {
    let stret_ptr = this.to_bits();
    let obj = env.objc.borrow::<CMDeviceMotionHostObject>(this);
    let (x, y, z) = (obj.ua_x, obj.ua_y, obj.ua_z);
    drop(obj);
    env.mem.write(Ptr::from_bits(stret_ptr),      x);
    env.mem.write(Ptr::from_bits(stret_ptr + 8),  y);
    env.mem.write(Ptr::from_bits(stret_ptr + 16), z);
}

// attitude — return a CMAttitude object
- (id)attitude {
    let obj = env.objc.borrow::<CMDeviceMotionHostObject>(this);
    let (roll, pitch, yaw) = (obj.roll, obj.pitch, obj.yaw);
    drop(obj);
    let att: id = msg_class![env; CMAttitude alloc];
    let att: id = msg![env; att init];
    *env.objc.borrow_mut::<CMAttitudeHostObject>(att) = CMAttitudeHostObject { roll, pitch, yaw };
    att
}

@end

// ── CMAttitude ────────────────────────────────────────────────────────────────

@implementation CMAttitude: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CMAttitudeHostObject { roll: 0.0, pitch: 0.0, yaw: 0.0 }), &mut env.mem)
}
- (id)init { this }

- (f64)roll  { env.objc.borrow::<CMAttitudeHostObject>(this).roll }
- (f64)pitch { env.objc.borrow::<CMAttitudeHostObject>(this).pitch }
- (f64)yaw   { env.objc.borrow::<CMAttitudeHostObject>(this).yaw }

// rotationMatrix (stret: 9× f64 = CMRotationMatrix)
- (())rotationMatrix {
    let stret_ptr = this.to_bits();
    // Identity matrix — good enough for games that just check it's non-zero
    let identity: [f64; 9] = [1.0,0.0,0.0, 0.0,1.0,0.0, 0.0,0.0,1.0];
    for (i, &v) in identity.iter().enumerate() {
        env.mem.write(Ptr::from_bits(stret_ptr + (i * 8) as u32), v);
    }
}

// quaternion (stret: 4× f64 = CMQuaternion)
- (())quaternion {
    let stret_ptr = this.to_bits();
    // Identity quaternion (x=0,y=0,z=0,w=1)
    let q: [f64; 4] = [0.0, 0.0, 0.0, 1.0];
    for (i, &v) in q.iter().enumerate() {
        env.mem.write(Ptr::from_bits(stret_ptr + (i * 8) as u32), v);
    }
}

@end

};
