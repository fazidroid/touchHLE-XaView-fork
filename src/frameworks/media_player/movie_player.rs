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
    id, msg, msg_class, nil, objc_classes, release, retain, todo_objc_setter, ClassExports,
    HostObject, NSZonePtr,
};
use crate::Environment;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    active_player: Option<id>,
    /// Various apps (e.g. Crash Bandicoot Nitro Kart 3D and Spore Origins)
    /// create or start a player and await some kind of notification, but can't
    /// handle it if that notification happens immediately. This queue lets us
    /// delay such notifications until the app next returns to the run loop,
    /// which seems to be late enough.
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

pub const MPMovieDurationAvailableNotification: &str =
    "MPMovieDurationAvailableNotification";
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
        "_MPMovieDurationAvailableNotification",
        HostConstant::NSString(MPMovieDurationAvailableNotification),
    ),
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
}
impl HostObject for MPMoviePlayerControllerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMoviePlayerController: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(MPMoviePlayerControllerHostObject {
        content_url: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithContentURL:(id)url { 
    log!(
        "TODO: [(MPMoviePlayerController*){:?} initWithContentURL:{:?} ({:?})]",
        this, url, ns_url::to_rust_path(env, url),
    );
    retain(env, url);
    env.objc.borrow_mut::<MPMoviePlayerControllerHostObject>(this).content_url = url;

    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerContentPreloadDidFinishNotification, this, Instant::now())
    );
    this
}

- (())dealloc {
    let url = env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).content_url;
    release(env, url);
    env.objc.dealloc_object(this, &mut env.mem);
}

- (id)contentURL {
    env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).content_url
}

// Real Racing 2 sets contentURL via property setter (alloc/init, then setContentURL:)
// rather than using initWithContentURL:. We fire all skip notifications here so
// the game's observers receive them regardless of which path was used.
- (())setContentURL:(id)url {
    env.objc.borrow_mut::<MPMoviePlayerControllerHostObject>(this).content_url = url;
    log_dbg!("MPMoviePlayerController setContentURL: firing instant-completion notifications");
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    // Post with both object:this and object:nil to catch all observer registrations.
    let senders: [id; 2] = [this, nil];
    for &sender in &senders {
        let n = ns_string::get_static_str(env, MPMoviePlayerLoadStateDidChangeNotification);
        let _: () = msg![env; center postNotificationName:n object:sender];
        let n = ns_string::get_static_str(env, MPMoviePlayerPlaybackDidFinishNotification);
        let _: () = msg![env; center postNotificationName:n object:sender];
    }
}

// Some games call prepareToPlay before play — treat as no-op but fire notifications.
- (())prepareToPlay {
    log_dbg!("MPMoviePlayerController prepareToPlay: firing instant-completion notifications");
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    let senders: [id; 2] = [this, nil];
    for &sender in &senders {
        let n = ns_string::get_static_str(env, MPMoviePlayerLoadStateDidChangeNotification);
        let _: () = msg![env; center postNotificationName:n object:sender];
        let n = ns_string::get_static_str(env, MPMoviePlayerPlaybackDidFinishNotification);
        let _: () = msg![env; center postNotificationName:n object:sender];
    }
}

- (id)backgroundColor { msg_class![env; UIColor blackColor] }
- (())setBackgroundColor:(id)color { todo_objc_setter!(this, color); }
- (())setScalingMode:(MPMovieScalingMode)mode { todo_objc_setter!(this, mode); }
- (())setUseApplicationAudioSession:(bool)use_session { todo_objc_setter!(this, use_session); }
- (())setControlStyle:(MPMovieControlStyle)style { todo_objc_setter!(this, style); }
- (())setFullscreen:(bool)fullsreen { todo_objc_setter!(this, fullsreen); }

- (id)view { nil }

- (MPMoviePlaybackState)playbackState { MPMoviePlaybackStateStopped }

- (())setMovieControlMode:(NSInteger)_mode {
    if env.bundle.bundle_identifier().starts_with("com.ea.spore") {
        log!("Applying game-specific hack for Spore Origins: sending MPMoviePlayerPlaybackDidFinishNotification again.");
        State::get(env).pending_notifications.push_back(
            (MPMoviePlayerPlaybackDidFinishNotification, this, Instant::now())
        );
    }
}

- (())setOrientation:(UIDeviceOrientation)_orientation animated:(bool)_animated { }

- (())play {
    let mut is_ea_game = false;
    if !env.is_app_picker {
        let bundle_id = env.bundle.bundle_identifier();
        is_ea_game = bundle_id.starts_with("com.ea") || bundle_id.starts_with("com.firemint");
    }

    if is_ea_game {
        println!("🎮 LOG: EA Title Detected! Faking instant video completion for MPMoviePlayerController...");
        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        
        let duration_notif = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMovieDurationAvailableNotification".to_string());
        let _: () = msg![env; center postNotificationName:duration_notif object:this];

        let finish_notif = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackDidFinishNotification".to_string());
        let _: () = msg![env; center postNotificationName:finish_notif object:this];
    } else {
        println!("🎮 LOG: Standard MPMoviePlayerController play called.");
        if let Some(old) = env.framework_state.media_player.movie_player.active_player {
            let _: () = msg![env; old stop];
        }
        assert!(env.framework_state.media_player.movie_player.active_player.is_none());
        retain(env, this);
        env.framework_state.media_player.movie_player.active_player = Some(this);

        State::get(env).pending_notifications.push_back(
            (MPMoviePlayerPlaybackDidFinishNotification, this, Instant::now())
        );
    }
}

- (())pause {
    log!("TODO: [(MPMoviePlayerController*){:?} pause]", this);
}

- (())stop {
    log!("TODO: [(MPMoviePlayerController*){:?} stop]", this);
    if env.framework_state.media_player.movie_player.active_player.is_some() {
        assert!(this == env.framework_state.media_player.movie_player.active_player.take().unwrap());
        release(env, this);
    }
}

@end

@implementation MPMoviePlayerViewController: UIViewController

- (id)initWithContentURL:(id)url {
    let mut is_ea_game = false;
    if !env.is_app_picker {
        let bundle_id = env.bundle.bundle_identifier();
        is_ea_game = bundle_id.starts_with("com.ea") || bundle_id.starts_with("com.firemint");
    }

    // 🏎️ DYNAMIC SPLIT: Real Racing 2 uses this, Gameloft gets nil!
    if is_ea_game {
        log_dbg!("MPMoviePlayerViewController initWithContentURL: EA/Firemint faking instant completion");
        let this: id = crate::msg_super![env; this init];
        if this == nil { return nil; }

        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        
        let load_notif = ns_string::get_static_str(env, MPMoviePlayerLoadStateDidChangeNotification);
        let _: () = msg![env; center postNotificationName:load_notif object:this];

        let duration_notif = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMovieDurationAvailableNotification".to_string());
        let _: () = msg![env; center postNotificationName:duration_notif object:this];

        let finish_notif = ns_string::get_static_str(env, MPMoviePlayerPlaybackDidFinishNotification);
        let _: () = msg![env; center postNotificationName:finish_notif object:this];

        return this;
    } else {
        log!(
            "TODO: [(MPMoviePlayerViewController*){:?} initWithContentURL:{:?} ({:?})] -> nil (Gameloft Bypass)",
            this, url, ns_url::to_rust_path(env, url),
        );
        release(env, this);
        return nil;
    }
}

- (id)moviePlayer { this }

- (())play {
    println!("🎮 LOG: Caught [MPMoviePlayerViewController play].");
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    let finish_notif = crate::frameworks::foundation::ns_string::get_static_str(env, MPMoviePlayerPlaybackDidFinishNotification);
    let _: () = msg![env; center postNotificationName:finish_notif object:this];
}

- (())stop { }
- (())pause { }
- (())setControlStyle:(i32)_style { }
- (())setScalingMode:(i32)_mode { }
- (())setFullscreen:(bool)_fullscreen animated:(bool)_animated { }
- (())setFullscreen:(bool)_fullscreen { }

// ==========================================================
// 🏎️ THE MOVIE CONFIGURATION GAUNTLET: Absorb everything!
// ==========================================================
- (())setMovieSourceType:(i32)source_type { }
- (())setInitialPlaybackTime:(f64)time { }
- (())setEndPlaybackTime:(f64)time { }
- (())setShouldAutoplay:(bool)autoplay { }
- (())setRepeatMode:(i32)mode { }
- (i32)loadState { 3 }
- (())setUseApplicationAudioSession:(bool)use_session { }

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
