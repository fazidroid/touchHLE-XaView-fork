/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIDevice`.

use crate::dyld::ConstantExports;
use crate::dyld::HostConstant;
use crate::frameworks::foundation::{ns_string, NSInteger};
// FixMsgClassImport
use crate::objc::{id, msg, msg_class, objc_classes, todo_objc_setter, ClassExports, TrivialHostObject};
use crate::window::DeviceOrientation;

pub const UIDeviceOrientationDidChangeNotification: &str =
    "UIDeviceOrientationDidChangeNotification";

pub type UIDeviceOrientation = NSInteger;
#[allow(dead_code)]
pub const UIDeviceOrientationUnknown: UIDeviceOrientation = 0;
pub const UIDeviceOrientationPortrait: UIDeviceOrientation = 1;
#[allow(dead_code)]
pub const UIDeviceOrientationPortraitUpsideDown: UIDeviceOrientation = 2;
pub const UIDeviceOrientationLandscapeLeft: UIDeviceOrientation = 3;
pub const UIDeviceOrientationLandscapeRight: UIDeviceOrientation = 4;
#[allow(dead_code)]
pub const UIDeviceOrientationFaceUp: UIDeviceOrientation = 5;
#[allow(dead_code)]
pub const UIDeviceOrientationFaceDown: UIDeviceOrientation = 6;

pub type UIDeviceBatteryState = NSInteger;
#[allow(dead_code)]
pub const UIDeviceBatteryStateUnknown: UIDeviceBatteryState = 0;
#[allow(dead_code)]
pub const UIDeviceBatteryStateUnplugged: UIDeviceBatteryState = 1;
#[allow(dead_code)]
pub const UIDeviceBatteryStateCharging: UIDeviceBatteryState = 2;
pub const UIDeviceBatteryStateFull: UIDeviceBatteryState = 3;

#[derive(Default)]
pub struct State {
    current_device: Option<id>,
}

pub const CONSTANTS: ConstantExports = &[(
    "_UIDeviceOrientationDidChangeNotification",
    HostConstant::NSString(UIDeviceOrientationDidChangeNotification),
)];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIDevice: NSObject

+ (id)currentDevice {
    if let Some(device) = env.framework_state.uikit.ui_device.current_device {
        device
    } else {
        let new = env.objc.alloc_static_object(
            this,
            Box::new(TrivialHostObject),
            &mut env.mem
        );
        env.framework_state.uikit.ui_device.current_device = Some(new);
        new
    }
}

- (())beginGeneratingDeviceOrientationNotifications {
    log!("TODO: beginGeneratingDeviceOrientationNotifications");
}
- (())endGeneratingDeviceOrientationNotifications {
    log!("TODO: endGeneratingDeviceOrientationNotifications");
}

- (id)model {
    ns_string::get_static_str(env, "iPhone")
}
- (id)localizedModel {
    msg![env; this model]
}

- (id)name {
    ns_string::get_static_str(env, "iPhone")
}

- (id)systemName {
    ns_string::get_static_str(env, "iPhone OS")
}

// ==========================================================
// 🏎️ EA BYPASS: Upgrade from iOS 2.0 to iOS 5.1.1
// ==========================================================
- (id)systemVersion {
    ns_string::get_static_str(env, "5.1.1")
}

- (NSInteger)userInterfaceIdiom {
    0 // UIUserInterfaceIdiomPhone
}

- (id)uniqueIdentifier {
    ns_string::get_static_str(env, "touchHLEdevice..........................")
}

- (id)identifierForVendor {
    msg_class![env; NSUUID UUID]
}

- (bool)isMultitaskingSupported {
    false
}
// EA games sometimes check for proximity sensor
- (bool)proximityMonitoringEnabled {
    false
}
- (())setProximityMonitoringEnabled:(bool)_enabled {
    // no-op
}
- (bool)proximityState {
    false
}

// Some games call this to detect if it's an iPod touch
- (id)model {
    ns_string::get_static_str(env, "iPhone")
}
// Add a specific method for "platform" if needed (though it's usually from sysctl)
- (id)platform {
    // EA custom method sometimes used
    ns_string::get_static_str(env, "iPhone4,1")
}
- (id)systemVersion {
    ns_string::get_static_str(env, "4.0")   // matches kern.osrelease 10.0.0d3
}

- (UIDeviceOrientation)orientation {
    match env.window().current_rotation() {
        DeviceOrientation::Portrait => UIDeviceOrientationPortrait,
        DeviceOrientation::LandscapeLeft => UIDeviceOrientationLandscapeLeft,
        DeviceOrientation::LandscapeRight => UIDeviceOrientationLandscapeRight
    }
}
- (())setOrientation:(UIDeviceOrientation)orientation {
    let rotation = match orientation {
        UIDeviceOrientationPortrait      => DeviceOrientation::Portrait,
        UIDeviceOrientationLandscapeLeft  => DeviceOrientation::LandscapeLeft,
        UIDeviceOrientationLandscapeRight => DeviceOrientation::LandscapeRight,
        _ => {
            log!("Warning: setOrientation: unhandled orientation {}, ignoring", orientation);
            return;
        }
    };
    env.window_mut().rotate_device(rotation);
}

- (bool)isBatteryMonitoringEnabled {
    true
}
- (())setBatteryMonitoringEnabled:(bool)_enabled {
    // No-op: battery monitoring not needed for emulation.
}
- (f32)batteryLevel {
    1.0
}
- (UIDeviceBatteryState)batteryState {
    UIDeviceBatteryStateFull
}

@end

@implementation CTTelephonyNetworkInfo: NSObject

+ (id)allocWithZone:(crate::objc::NSZonePtr)_zone {
    let host_object = Box::new(TrivialHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    this
}

- (id)subscriberCellularProvider {
    let carrier: id = msg_class![env; CTCarrier alloc];
    let carrier: id = msg![env; carrier init];
    crate::objc::autorelease(env, carrier)
}

@end

@implementation CTCarrier: NSObject

+ (id)allocWithZone:(crate::objc::NSZonePtr)_zone {
    let host_object = Box::new(TrivialHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    this
}

- (id)carrierName {
    ns_string::get_static_str(env, "touchHLE")
}

- (id)mobileCountryCode {
    ns_string::get_static_str(env, "310")
}

- (id)mobileNetworkCode {
    ns_string::get_static_str(env, "410")
}

- (id)isoCountryCode {
    ns_string::get_static_str(env, "us")
}

- (bool)allowsVOIP {
    true
}

@end

@implementation UIPasteboard: NSObject

+ (id)allocWithZone:(crate::objc::NSZonePtr)_zone {
    let host_object = Box::new(TrivialHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)pasteboardWithName:(id)_name create:(bool)_create {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    crate::objc::autorelease(env, new)
}

+ (id)generalPasteboard {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    crate::objc::autorelease(env, new)
}

- (id)init {
    this
}

- (id)string {
    ns_string::get_static_str(env, "")
}

- (())setString:(id)_string {}

- (id)dataForPasteboardType:(id)_pasteboardType {
    crate::objc::nil
}

- (())setData:(id)_data forPasteboardType:(id)_pasteboardType {}

- (id)valueForPasteboardType:(id)_pasteboardType {
    crate::objc::nil
}

- (())setValue:(id)_value forPasteboardType:(id)_pasteboardType {}

@end

};