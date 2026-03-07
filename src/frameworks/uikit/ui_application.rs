/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIApplication` and `UIApplicationMain`.

use super::ui_device::*;
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str};
use crate::frameworks::foundation::{ns_array, ns_string, NSInteger, NSUInteger};
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::window::DeviceOrientation;
use crate::{todo_objc_setter, Environment};

#[derive(Default)]
pub struct State {
    shared_application: Option<id>,
    pub(super) status_bar_hidden: bool,
}

struct UIApplicationHostObject {
    delegate: id,
    delegate_is_retained: bool,
}
impl HostObject for UIApplicationHostObject {}

// Добавляем память для наших уведомлений
struct UILocalNotificationHostObject {
    fire_date: id,
    time_zone: id,
    alert_body: id,
    alert_action: id,
    sound_name: id,
    user_info: id,
    badge_number: NSInteger,
    repeat_interval: NSInteger,
}
impl Default for UILocalNotificationHostObject {
    fn default() -> Self {
        Self {
            fire_date: nil, time_zone: nil, alert_body: nil,
            alert_action: nil, sound_name: nil, user_info: nil,
            badge_number: 0, repeat_interval: 0,
        }
    }
}
impl HostObject for UILocalNotificationHostObject {}

pub type UIInterfaceOrientation = UIDeviceOrientation;
#[allow(unused)]
pub const UIInterfaceOrientationPortrait: UIInterfaceOrientation = UIDeviceOrientationPortrait;
#[allow(unused)]
pub const UIInterfaceOrientationPortraitUpsideDown: UIInterfaceOrientation =
    UIDeviceOrientationPortraitUpsideDown;
pub const UIInterfaceOrientationLandscapeLeft: UIInterfaceOrientation =
    UIDeviceOrientationLandscapeRight;
pub const UIInterfaceOrientationLandscapeRight: UIInterfaceOrientation =
    UIDeviceOrientationLandscapeLeft;

type UIRemoteNotificationType = NSUInteger;
type UIStatusBarAnimation = NSInteger;
type UIStatusBarStyle = NSInteger;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIApplication: UIResponder

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIApplicationHostObject {
        delegate: nil,
        delegate_is_retained: false,
    });
    env.objc.alloc_static_object(this, host_object, &mut env.mem)
}

+ (id)sharedApplication {
    if let Some(app) = env.framework_state.uikit.ui_application.shared_application {
        return app;
    }
    let class = env.objc.get_known_class("UIApplication", &mut env.mem);
    let app: id = msg![env; class alloc];
    let app_init: id = msg![env; app init];
    env.framework_state.uikit.ui_application.shared_application = Some(app_init);
    app_init
}

- (id)init {
    env.framework_state.uikit.ui_application.shared_application = Some(this);
    this
}

- (id)retain { this }
- (id)autorelease { this }
- (())release {}

- (id)delegate {
    env.objc.borrow::<UIApplicationHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    let host_object = env.objc.borrow_mut::<UIApplicationHostObject>(this);
    let old_delegate = std::mem::replace(&mut host_object.delegate, delegate);
    if host_object.delegate_is_retained {
        host_object.delegate_is_retained = false;
        if delegate != old_delegate {
            release(env, old_delegate);
        }
    }
}

- (bool)isStatusBarHidden {
    env.framework_state.uikit.ui_application.status_bar_hidden
}
- (())setStatusBarHidden:(bool)hidden {
    env.framework_state.uikit.ui_application.status_bar_hidden = hidden;
}
- (())setStatusBarHidden:(bool)hidden animated:(bool)_animated {
    msg![env; this setStatusBarHidden:hidden]
}
- (())setStatusBarHidden:(bool)hidden withAnimation:(UIStatusBarAnimation)_animation {
    msg![env; this setStatusBarHidden:hidden]
}

- (())setStatusBarStyle:(UIStatusBarStyle)style {
    todo_objc_setter!(this, style);
}

- (UIInterfaceOrientation)statusBarOrientation {
    match env.window().current_rotation() {
        DeviceOrientation::Portrait => UIDeviceOrientationPortrait,
        DeviceOrientation::LandscapeLeft => UIDeviceOrientationLandscapeLeft,
        DeviceOrientation::LandscapeRight => UIDeviceOrientationLandscapeRight
    }
}
- (())setStatusBarOrientation:(UIInterfaceOrientation)orientation {
    env.window_mut().rotate_device(match orientation {
        UIDeviceOrientationPortrait => DeviceOrientation::Portrait,
        UIDeviceOrientationLandscapeLeft => DeviceOrientation::LandscapeLeft,
        UIDeviceOrientationLandscapeRight => DeviceOrientation::LandscapeRight,
        _ => unimplemented!("Orientation {} not handled yet", orientation),
    });
}
- (())setStatusBarOrientation:(UIInterfaceOrientation)orientation animated:(bool)_animated {
    msg![env; this setStatusBarOrientation:orientation]
}

- (bool)isIdleTimerDisabled {
    !env.window().is_screen_saver_enabled()
}
- (())setIdleTimerDisabled:(bool)disabled {
    env.window_mut().set_screen_saver_enabled(!disabled);
}

- (bool)openURL:(id)url {
    let ns_string = msg![env; url absoluteString];
    let url_string = ns_string::to_rust_string(env, ns_string);
    if let Err(e) = crate::window::open_url(&url_string) {
        echo!("App opened URL {:?} unsuccessfully ({}), exiting.", url_string, e);
    } else {
        echo!("App opened URL {:?}, exiting.", url_string);
    }
    exit(env);
    true
}

-(())beginIgnoringInteractionEvents {
    log!("TODO: ignoring beginIgnoringInteractionEvents");
}
- (bool)isIgnoringInteractionEvents {
    false
}
-(())endIgnoringInteractionEvents {
    log!("TODO: ignoring endIgnoringInteractionEvents");
}

- (id)keyWindow {
    let Some(key_window) = env.framework_state.uikit.ui_view.ui_window.key_window else {
        return nil;
    };
    key_window
}

- (id)windows {
    let windows: Vec<id> = (*env.framework_state.uikit.ui_view.ui_window.windows).to_vec();
    for window in &windows {
        retain(env, *window);
    }
    let windows = ns_array::from_vec(env, windows);
    autorelease(env, windows)
}

- (())registerForRemoteNotificationTypes:(UIRemoteNotificationType)types {
    log!("TODO: ignoring registerForRemoteNotificationTypes:{}", types);
}

- (NSInteger)applicationIconBadgeNumber {
    0
}
- (())setApplicationIconBadgeNumber:(NSInteger)bn {
    log!("TODO: ignoring setApplicationIconBadgeNumber:{}", bn);
}

- (id)scheduledLocalNotifications {
    let class = env.objc.get_known_class("NSArray", &mut env.mem);
    let arr: id = msg![env; class alloc];
    let arr_init: id = msg![env; arr init];
    autorelease(env, arr_init)
}

- (NSInteger)applicationState {
    0 // UIApplicationStateActive
}

- (())setScheduledLocalNotifications:(id)_notifications {
    log!("TODO: ignoring setScheduledLocalNotifications");
}
- (())cancelAllLocalNotifications {
    log!("TODO: [UIApplication cancelAllLocalNotifications]");
}
- (())cancelLocalNotification:(id)_notification {
    log!("TODO: [UIApplication cancelLocalNotification]");
}
- (())scheduleLocalNotification:(id)_notification {
    log!("TODO: [UIApplication scheduleLocalNotification]");
}

- (id)nextResponder {
    let delegate = msg![env; this delegate];
    let app_delegate_class = msg![env; delegate class];
    let ui_responder_class = env.objc.get_known_class("UIResponder", &mut env.mem);
    if env.objc.class_is_subclass_of(app_delegate_class, ui_responder_class) {
        delegate
    } else {
        nil
    }
}

@end

@implementation UILocalNotification: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UILocalNotificationHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}
- (id)init {
    let empty_str = get_static_str(env, "");
    let empty_dict = msg_class![env; NSDictionary dictionary];

    {
        // Изолируем мутабельное заимствование в блоке
        let mut host = env.objc.borrow_mut::<UILocalNotificationHostObject>(this);
        host.alert_body = empty_str;
        host.alert_action = empty_str;
        host.sound_name = empty_str;
        host.user_info = empty_dict;
    }

    // Увеличиваем счетчик ссылок, так как наш dealloc потом их отпустит
    retain(env, empty_str);
    retain(env, empty_str);
    retain(env, empty_str);
    retain(env, empty_dict);

    this
}
- (())dealloc {
    let &UILocalNotificationHostObject {
        fire_date,
        time_zone,
        alert_body,
        alert_action,
        sound_name,
        user_info,
        badge_number: _,
        repeat_interval: _,
    } = env.objc.borrow(this);

    release(env, fire_date);
    release(env, time_zone);
    release(env, alert_body);
    release(env, alert_action);
    release(env, sound_name);
    release(env, user_info);

    env.objc.dealloc_object(this, &mut env.mem)
}

- (())setFireDate:(id)val {
    let old = std::mem::replace(&mut env.objc.borrow_mut::<UILocalNotificationHostObject>(this).fire_date, val);
    retain(env, val); release(env, old);
}
- (id)fireDate { env.objc.borrow::<UILocalNotificationHostObject>(this).fire_date }

- (())setTimeZone:(id)val {
    let old = std::mem::replace(&mut env.objc.borrow_mut::<UILocalNotificationHostObject>(this).time_zone, val);
    retain(env, val); release(env, old);
}
- (id)timeZone { env.objc.borrow::<UILocalNotificationHostObject>(this).time_zone }

- (())setAlertBody:(id)val {
    let old = std::mem::replace(&mut env.objc.borrow_mut::<UILocalNotificationHostObject>(this).alert_body, val);
    retain(env, val); release(env, old);
}
- (id)alertBody { env.objc.borrow::<UILocalNotificationHostObject>(this).alert_body }

- (())setAlertAction:(id)val {
    let old = std::mem::replace(&mut env.objc.borrow_mut::<UILocalNotificationHostObject>(this).alert_action, val);
    retain(env, val); release(env, old);
}
- (id)alertAction { env.objc.borrow::<UILocalNotificationHostObject>(this).alert_action }

- (())setSoundName:(id)val {
    let old = std::mem::replace(&mut env.objc.borrow_mut::<UILocalNotificationHostObject>(this).sound_name, val);
    retain(env, val); release(env, old);
}
- (id)soundName { env.objc.borrow::<UILocalNotificationHostObject>(this).sound_name }

- (())setUserInfo:(id)val {
    let old = std::mem::replace(&mut env.objc.borrow_mut::<UILocalNotificationHostObject>(this).user_info, val);
    retain(env, val); release(env, old);
}
- (id)userInfo { env.objc.borrow::<UILocalNotificationHostObject>(this).user_info }

- (())setApplicationIconBadgeNumber:(NSInteger)val {
    env.objc.borrow_mut::<UILocalNotificationHostObject>(this).badge_number = val;
}
- (NSInteger)applicationIconBadgeNumber { env.objc.borrow::<UILocalNotificationHostObject>(this).badge_number }

- (())setRepeatInterval:(NSInteger)val {
    env.objc.borrow_mut::<UILocalNotificationHostObject>(this).repeat_interval = val;
}
- (NSInteger)repeatInterval { env.objc.borrow::<UILocalNotificationHostObject>(this).repeat_interval }

@end

};

pub(super) fn UIApplicationMain(
    env: &mut Environment,
    _argc: i32,
    _argv: MutPtr<MutPtr<u8>>,
    principal_class_name: id,
    delegate_class_name: id,
) {
    let ui_application = {
        let pool: id = msg_class![env; NSAutoreleasePool new];

        let principal_class = if principal_class_name != nil {
            let name = ns_string::to_rust_string(env, principal_class_name);
            env.objc.get_known_class(&name, &mut env.mem)
        } else {
            env.objc.get_known_class("UIApplication", &mut env.mem)
        };
        let ui_application: id = msg![env; principal_class new];

        let device_family = env.options.device_family;
        if let Some(main_nib_filename) = env.bundle.main_nib_filename(device_family) {
            let ns_main_nib_filename = from_rust_string(env, main_nib_filename.to_string());
            let type_: id = get_static_str(env, "nib");
            let bundle: id = msg_class![env; NSBundle mainBundle];
            let res: id = msg![env; bundle pathForResource:ns_main_nib_filename ofType:type_];
            if res != nil {
                let nib: id = msg_class![env; UINib nibWithNibName:ns_main_nib_filename bundle:nil];
                release(env, ns_main_nib_filename);
                let _: id = msg![env; nib instantiateWithOwner:ui_application options:nil];
            } else {
                log!("Warning: couldn't load main nib file {:?}", env.bundle.main_nib_filename(device_family));
            }
        }

        if env.bundle.status_bar_hidden() {
            let _: () = msg![env; ui_application setStatusBarHidden:true];
        }

        let delegate: id = msg![env; ui_application delegate];
        if delegate != nil {
            env.objc.borrow_mut::<UIApplicationHostObject>(ui_application).delegate_is_retained = true;
            retain(env, delegate);
        } else {
            assert!(delegate_class_name != nil);
            if msg![env; delegate_class_name isEqual:principal_class_name] {
                let _: () = msg![env; ui_application setDelegate:ui_application];
            } else {
                let name = ns_string::to_rust_string(env, delegate_class_name);
                let class = env.objc.get_known_class(&name, &mut env.mem);
                let delegate: id = msg![env; class new];
                let _: () = msg![env; ui_application setDelegate:delegate];
                assert!(delegate != nil);
            }
        };

        let _: () = msg![env; pool drain];

        ui_application
    };

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        if env.objc.object_has_method_named(&env.mem, delegate, "application:didFinishLaunchingWithOptions:") {
            let empty_dict: id = msg_class![env; NSDictionary dictionary];
            () = msg![env; delegate application:ui_application didFinishLaunchingWithOptions:empty_dict];
        } else if env.objc.object_has_method_named(&env.mem, delegate, "applicationDidFinishLaunching:") {
            () = msg![env; delegate applicationDidFinishLaunching:ui_application];
        }

        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let notif_name = get_static_str(env, UIApplicationDidFinishLaunchingNotification);
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];

        let _: () = msg![env; pool drain];
    }

    let views = env.framework_state.uikit.ui_view.views.clone();
    for view in views {
        () = msg![env; view layoutSubviews];
    }

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        if env.objc.object_has_method_named(&env.mem, delegate, "applicationDidBecomeActive:") {
            () = msg![env; delegate applicationDidBecomeActive:ui_application];
        }

        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let notif_name = get_static_str(env, UIApplicationDidBecomeActiveNotification);
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];

        let _: () = msg![env; pool drain];
    }

    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    let _: () = msg![env; run_loop run];
}

pub(super) fn exit(env: &mut Environment) {
    let ui_application: id = msg_class![env; UIApplication sharedApplication];
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        if !env.is_fake {
            let user_defaults: id = msg_class![env; NSUserDefaults standardUserDefaults];
            let _: bool = msg![env; user_defaults synchronize];
        }
        let delegate: id = msg![env; ui_application delegate];
        if env.objc.object_has_method_named(&env.mem, delegate, "applicationWillResignActive:") {
            () = msg![env; delegate applicationWillResignActive:ui_application];
        }
        let notif_name = get_static_str(env, UIApplicationWillResignActiveNotification);
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];
        let _: () = msg![env; pool drain];
    };

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        if env.objc.object_has_method_named(&env.mem, delegate, "applicationWillTerminate:") {
            () = msg![env; delegate applicationWillTerminate:ui_application];
        }
        let notif_name = get_static_str(env, UIApplicationWillTerminateNotification);
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];
        let _: () = msg![env; pool drain];
    };

    std::process::exit(0);
}

const UIApplicationDidFinishLaunchingNotification: &str = "UIApplicationDidFinishLaunchingNotification";
const UIApplicationDidBecomeActiveNotification: &str = "UIApplicationDidBecomeActiveNotification";
const UIApplicationDidEnterBackgroundNotification: &str = "UIApplicationDidEnterBackgroundNotification";
const UIApplicationWillEnterForegroundNotification: &str = "UIApplicationWillEnterForegroundNotification";
const UIApplicationWillResignActiveNotification: &str = "UIApplicationWillResignActiveNotification";
const UIApplicationWillTerminateNotification: &str = "UIApplicationWillTerminateNotification";
const UIApplicationLaunchOptionsRemoteNotificationKey: &str = "UIApplicationLaunchOptionsRemoteNotificationKey";
const UIApplicationDidReceiveMemoryWarningNotification: &str = "UIApplicationDidReceiveMemoryWarningNotification";

pub const CONSTANTS: ConstantExports = &[
    ("_UIApplicationDidFinishLaunchingNotification", HostConstant::NSString(UIApplicationDidFinishLaunchingNotification)),
    ("_UIApplicationDidBecomeActiveNotification", HostConstant::NSString(UIApplicationDidBecomeActiveNotification)),
    ("_UIApplicationDidEnterBackgroundNotification", HostConstant::NSString(UIApplicationDidEnterBackgroundNotification)),
    ("_UIApplicationWillEnterForegroundNotification", HostConstant::NSString(UIApplicationWillEnterForegroundNotification)),
    ("_UIApplicationWillResignActiveNotification", HostConstant::NSString(UIApplicationWillResignActiveNotification)),
    ("_UIApplicationWillTerminateNotification", HostConstant::NSString(UIApplicationWillTerminateNotification)),
    ("_UIApplicationDidReceiveMemoryWarningNotification", HostConstant::NSString(UIApplicationDidReceiveMemoryWarningNotification)),
    ("_UIApplicationLaunchOptionsRemoteNotificationKey", HostConstant::NSString(UIApplicationLaunchOptionsRemoteNotificationKey)),
];

pub const FUNCTIONS: FunctionExports = &[export_c_func!(UIApplicationMain(_, _, _, _))];