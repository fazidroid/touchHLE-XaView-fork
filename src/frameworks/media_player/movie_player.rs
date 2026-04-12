/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMoviePlayerController` etc.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::{ns_string, ns_url, NSInteger};
use crate::frameworks::uikit::ui_device::UIDeviceOrientation;
use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, retain, ClassExports,
    HostObject, NSZonePtr,
};
use crate::Environment;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    active_player: Option<id>,
    pending_notifications: VecDeque<(&'static str, id, Instant)>,
}
impl State {
    fn get(env: &mut Environment) -> &mut Self {
        &mut env.framework_state.media_player.movie_player
    }
}

type MPMovieScalingMode = NSInteger;
type MPMovieControlStyle = NSInteger;
type MPMoviePlaybackState = NSInteger;
const MPMoviePlaybackStateStopped: MPMoviePlaybackState = 0;

pub const MPMoviePlayerPlaybackDidFinishNotification: &str =
    "MPMoviePlayerPlaybackDidFinishNotification";
pub const MPMoviePlayerContentPreloadDidFinishNotification: &str =
    "MPMoviePlayerContentPreloadDidFinishNotification";
pub const MPMoviePlayerScalingModeDidChangeNotification: &str =
    "MPMoviePlayerScalingModeDidChangeNotification";
pub const MPMoviePlayerLoadStateDidChangeNotification: &str =
    "MPMoviePlayerLoadStateDidChangeNotification";
const MPMoviePlayerPlaybackDidFinishReasonUserInfoKey: &str =
    "MPMoviePlayerPlaybackDidFinishReasonUserInfoKey";

pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMoviePlayerPlaybackDidFinishNotification",
        HostConstant::NSString(MPMoviePlayerPlaybackDidFinishNotification),
    ),
    (
        "_MPMoviePlayerContentPreloadDidFinishNotification",
        HostConstant::NSString(MPMoviePlayerContentPreloadDidFinishNotification),
    ),
    (
        "_MPMoviePlayerScalingModeDidChangeNotification",
        HostConstant::NSString(MPMoviePlayerScalingModeDidChangeNotification),
    ),
    (
        "_MPMoviePlayerLoadStateDidChangeNotification",
        HostConstant::NSString(MPMoviePlayerLoadStateDidChangeNotification),
    ),
    (
        "_MPMoviePlayerPlaybackDidFinishReasonUserInfoKey",
        HostConstant::NSString(MPMoviePlayerPlaybackDidFinishReasonUserInfoKey),
    ),
];

struct MPMoviePlayerControllerHostObject {
    content_url: id,
    view: id,
}
impl HostObject for MPMoviePlayerControllerHostObject {}

struct MPMoviePlayerViewControllerHostObject {
    player: id,
}
impl HostObject for MPMoviePlayerViewControllerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMoviePlayerController: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(MPMoviePlayerControllerHostObject {
        content_url: nil,
        view: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithContentURL:(id)url {
    retain(env, url);
    
    let view: id = msg_class![env; UIView alloc];
    let view: id = msg![env; view init];
    
    // Make the dummy view 100% invisible so it doesn't block the 3D graphics
    let clear_color: id = msg_class![env; UIColor clearColor];
    let _: () = msg![env; view setBackgroundColor:clear_color];
    let _: () = msg![env; view setOpaque:false];
    let _: () = msg![env; view setHidden:true];
    
    let mut host_obj = env.objc.borrow_mut::<MPMoviePlayerControllerHostObject>(this);
    host_obj.content_url = url;
    host_obj.view = view;

    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerContentPreloadDidFinishNotification, this, Instant::now() + Duration::from_millis(50))
    );

    this
}

- (())dealloc {
    let url = env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).content_url;
    release(env, url);
    
    let view = env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).view;
    release(env, view);

    env.objc.dealloc_object(this, &mut env.mem);
}

- (id)contentURL {
    env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).content_url
}

- (id)backgroundColor {
    msg_class![env; UIColor clearColor]
}

- (())setBackgroundColor:(id)_color {}
- (())setScalingMode:(MPMovieScalingMode)_mode {}
- (())setUseApplicationAudioSession:(bool)_use_session {}
- (())setControlStyle:(MPMovieControlStyle)_style {}
- (())setFullscreen:(bool)_fullscreen {}

- (id)view {
    env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).view
}

// 🏎️ THE STRONG HACKS: Break the Gameloft polling loop!
- (NSInteger)loadState {
    // Return MPMovieLoadStatePlayable (1) | MPMovieLoadStatePlaythroughOK (2)
    3 
}

- (bool)isPreparedToPlay {
    true
}

- (())prepareToPlay {
    log!("🏎️ GAMELOFT BYPASS: [MPMoviePlayerController prepareToPlay] called!");
    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerLoadStateDidChangeNotification, this, Instant::now() + Duration::from_millis(50))
    );
}

- (MPMoviePlaybackState)playbackState {
    MPMoviePlaybackStateStopped
}

- (())setMovieControlMode:(NSInteger)_mode {}

- (())setOrientation:(UIDeviceOrientation)_orientation animated:(bool)_animated {}

- (())play {
    if let Some(old) = env.framework_state.media_player.movie_player.active_player {
        let _: () = msg![env; old stop];
    }
    assert!(env.framework_state.media_player.movie_player.active_player.is_none());
    retain(env, this);
    env.framework_state.media_player.movie_player.active_player = Some(this);

    let notif = (MPMoviePlayerPlaybackDidFinishNotification, this, Instant::now() + Duration::from_millis(100));
    for (name, obj, _) in &mut State::get(env).pending_notifications {
        if *name == MPMoviePlayerPlaybackDidFinishNotification && *obj == this {
            return;
        }
    }
    State::get(env).pending_notifications.push_back(notif);
}

- (())pause {}

- (())stop {
    if env.framework_state.media_player.movie_player.active_player.is_some() {
        assert!(this == env.framework_state.media_player.movie_player.active_player.take().unwrap());
        release(env, this);
    }
}

@end

@implementation MPMoviePlayerViewController: UIViewController

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(MPMoviePlayerViewControllerHostObject {
        player: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithContentURL:(id)url {
    let player: id = msg_class![env; MPMoviePlayerController alloc];
    let player: id = msg![env; player initWithContentURL:url];
    
    env.objc.borrow_mut::<MPMoviePlayerViewControllerHostObject>(this).player = player;
    
    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerPlaybackDidFinishNotification, player, Instant::now() + Duration::from_millis(500))
    );
    
    this
}

- (())dealloc {
    let player = env.objc.borrow::<MPMoviePlayerViewControllerHostObject>(this).player;
    release(env, player);
    env.objc.dealloc_object(this, &mut env.mem);
}

- (id)moviePlayer {
    env.objc.borrow::<MPMoviePlayerViewControllerHostObject>(this).player
}

@end

};

pub(super) fn handle_players(env: &mut Environment) {
    let mut notifs_to_run = Vec::new();
    let pending_notifs = &mut State::get(env).pending_notifications;
    let mut i = 0;
    while i < pending_notifs.len() {
        let (name_str, object, time) = pending_notifs[i];
        if Instant::now() >= time {
            notifs_to_run.push((name_str, object));
            pending_notifs.swap_remove_back(i);
        } else {
            i += 1;
        }
    }
    for (name_str, object) in notifs_to_run {
        let name = ns_string::get_static_str(env, name_str);
        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let _: () = msg![env; center postNotificationName:name object:object];
    }
}
