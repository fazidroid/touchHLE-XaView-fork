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
    // TODO: Hardcoded to iPhone for now
    ns_string::get_static_str(env, "iPhone")
}
- (id)localizedModel {
    // TODO: localization
    msg![env; this model]
}

- (id)name {
    // TODO: Hardcoded to iPhone for now
    ns_string::get_static_str(env, "iPhone")
}

- (id)systemName {
    ns_string::get_static_str(env, "iPhone OS")
}

// NSString
- (id)systemVersion {
    ns_string::get_static_str(env, "2.0")
}

- (id)uniqueIdentifier {
    // Aspen Simulator returns (null) here
    // A device unique identifier must be 40 characters long
    ns_string::get_static_str(env, "touchHLEdevice..........................")
}

- (id)identifierForVendor {
    // FakeVendorIdentifier
    msg_class![env; NSUUID UUID]
}

- (bool)isMultitaskingSupported {
    false
}

- (UIDeviceOrientation)orientation {
    match env.window().current_rotation() {
        DeviceOrientation::Portrait => UIDeviceOrientationPortrait,
        DeviceOrientation::LandscapeLeft => UIDeviceOrientationLandscapeLeft,
        DeviceOrientation::LandscapeRight => UIDeviceOrientationLandscapeRight
    }
}
- (())setOrientation:(UIDeviceOrientation)orientation {
    env.window_mut().rotate_device(match orientation {
        UIDeviceOrientationPortrait => DeviceOrientation::Portrait,
        UIDeviceOrientationLandscapeLeft => DeviceOrientation::LandscapeLeft,
        UIDeviceOrientationLandscapeRight => DeviceOrientation::LandscapeRight,
        _ => unimplemented!("Orientation {} not handled yet", orientation),
    });
}

- (bool)isBatteryMonitoringEnabled {
    true
}
- (())setBatteryMonitoringEnabled:(bool)enabled {
    todo_objc_setter!(this, enabled);
    assert!(enabled);
}
- (f32)batteryLevel {
    // BypassSDLCrash
    1.0
}
- (UIDeviceBatteryState)batteryState {
    // FakeBatteryFull
    UIDeviceBatteryStateFull
}

@end

@implementation CTTelephonyNetworkInfo: NSObject

+ (id)allocWithZone:(crate::objc::NSZonePtr)_zone {
    // FakeTelephonyAlloc
    let host_object = Box::new(TrivialHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    // FakeTelephonyInit
    this
}

- (id)subscriberCellularProvider {
    // FakeTelephonyProvider
    let carrier: id = msg_class![env; CTCarrier alloc];
    let carrier: id = msg![env; carrier init];
    crate::objc::autorelease(env, carrier)
}

@end

@implementation CTCarrier: NSObject

+ (id)allocWithZone:(crate::objc::NSZonePtr)_zone {
    // FakeCarrierAlloc
    let host_object = Box::new(TrivialHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    // FakeCarrierInit
    this
}

- (id)carrierName {
    // FakeCarrierName
    ns_string::get_static_str(env, "touchHLE")
}

- (id)mobileCountryCode {
    // FakeCarrierMCC
    ns_string::get_static_str(env, "310")
}

- (id)mobileNetworkCode {
    // FakeCarrierMNC
    ns_string::get_static_str(env, "410")
}

- (id)isoCountryCode {
    // FakeCarrierISO
    ns_string::get_static_str(env, "us")
}

- (bool)allowsVOIP {
    // FakeCarrierVOIP
    true
}

@end

@implementation UIPasteboard: NSObject

+ (id)allocWithZone:(crate::objc::NSZonePtr)_zone {
    // FakePasteboardAlloc
    let host_object = Box::new(TrivialHostObject);
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    // FakePasteboardInit
    this
}

+ (id)pasteboardWithName:(id)_name create:(bool)_create {
    // FakePasteboardWithName
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    crate::objc::autorelease(env, new)
}

+ (id)generalPasteboard {
    // FakeGeneralPasteboard
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    crate::objc::autorelease(env, new)
}

- (id)string {
    // FakePasteboardString
    ns_string::get_static_str(env, "")
}

- (())setString:(id)_string {
    // FakePasteboardSetString
}

- (id)dataForPasteboardType:(id)_pasteboardType {
    // FakePasteboardData
    crate::objc::nil
}

- (())setData:(id)_data forPasteboardType:(id)_pasteboardType {
    // FakePasteboardSetData
}

- (id)valueForPasteboardType:(id)_pasteboardType {
    // FakePasteboardValue
    crate::objc::nil
}

- (())setValue:(id)_value forPasteboardType:(id)_pasteboardType {
    // FakePasteboardSetValue
}

@end

};
