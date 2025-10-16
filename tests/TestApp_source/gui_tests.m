/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#include "system_headers.h"
#include <stdio.h>

@interface TestAppDelegate : NSObject
@end
@implementation TestAppDelegate : NSObject
UIWindow *window;
UIView *ball;
CGFloat ballXVelocity;
CGFloat ballYVelocity;
- (void)applicationDidFinishLaunching:(id)app {
  window =
      [[UIWindow alloc] initWithFrame:[[UIScreen mainScreen] applicationFrame]];
  UILabel *label = [[UILabel alloc] initWithFrame:[window bounds]];
  label.text = [NSString stringWithUTF8String:"hello, world! 🌏"];
  label.textAlignment = UITextAlignmentCenter;
  [window addSubview:label];
  ball = [[UIView alloc] initWithFrame:CGRectMake(0, 0, 20, 20)];
  ball.backgroundColor = [UIColor redColor];
  [window addSubview:ball];
  [window makeKeyAndVisible];

  ballXVelocity = 5;
  ballYVelocity = 5;
  [NSTimer scheduledTimerWithTimeInterval:(1.0 / 60.0)
                                   target:self
                                 selector:@selector(moveBall:)
                                 userInfo:nil
                                  repeats:YES];
}

- (void)moveBall:(NSTimer *)timer {
  CGRect windowFrame = [window bounds];
  CGRect ballFrame = [ball frame];
  ballFrame.origin.x += ballXVelocity;
  ballFrame.origin.y += ballYVelocity;
  CGFloat oldXVelocity = ballXVelocity;
  CGFloat oldYVelocity = ballYVelocity;
  if (CGRectGetMaxX(ballFrame) >= CGRectGetMaxX(windowFrame)) {
    ballXVelocity = -ballXVelocity;
    ballFrame.origin.x = CGRectGetMaxX(windowFrame) - ballFrame.size.width;
  } else if (CGRectGetMinX(ballFrame) <= CGRectGetMinX(windowFrame)) {
    ballXVelocity = -ballXVelocity;
    ballFrame.origin.x = CGRectGetMinX(windowFrame);
  }
  if (CGRectGetMaxY(ballFrame) >= CGRectGetMaxY(windowFrame)) {
    ballYVelocity = -ballYVelocity;
    ballFrame.origin.y = CGRectGetMaxY(windowFrame) - ballFrame.size.height;
  } else if (CGRectGetMinY(ballFrame) <= CGRectGetMinY(windowFrame)) {
    ballYVelocity = -ballYVelocity;
    ballFrame.origin.y = CGRectGetMinY(windowFrame);
  }
  if (oldXVelocity != ballXVelocity || oldYVelocity != ballYVelocity)
    ball.backgroundColor =
        [UIColor colorWithRed:(ballFrame.origin.x / windowFrame.size.width)
                        green:((ballXVelocity + ballYVelocity) / 10.0 + 0.5)
                         blue:(ballFrame.origin.y / windowFrame.size.height)
                        alpha:1.0];
  ball.frame = ballFrame;
}
@end

int TestApp_gui_tests_main(int argc, char **argv) {
  id pool = [NSAutoreleasePool new];
  int res = UIApplicationMain(argc, argv, NULL,
                              NSStringFromClass([TestAppDelegate class]));
  [pool release];
  return res;
}
