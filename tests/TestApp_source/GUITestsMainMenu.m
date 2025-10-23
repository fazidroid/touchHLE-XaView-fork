/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#include "system_headers.h"

#include "GUITestsAppDelegate.h"
#include "GUITestsCALayerTestsView.h"
#include "GUITestsMainMenu.h"

@implementation GUITestsMainMenu : UIView

UIView *ball;
CGFloat ballXVelocity;
CGFloat ballYVelocity;
NSTimer *timer;

- (instancetype)initWithFrame:(CGRect)frame {
  [super initWithFrame:frame];

  UILabel *label = [[[UILabel alloc] initWithFrame:[self bounds]] autorelease];
  label.text = [NSString stringWithUTF8String:"hello, world! 🌏"];
  label.textAlignment = UITextAlignmentCenter;
  [self addSubview:label];
  ball = [[UIView alloc] initWithFrame:CGRectMake(0, 0, 20, 20)];
  ball.layer.cornerRadius = ball.frame.size.width / 2;
  ball.backgroundColor = [UIColor redColor];
  [self addSubview:ball];
  ballXVelocity = 5;
  ballYVelocity = 5;
  timer = [[NSTimer scheduledTimerWithTimeInterval:(1.0 / 60.0)
                                            target:self
                                          selector:@selector(moveBall:)
                                          userInfo:nil
                                           repeats:YES] retain];

  UIButton *button1 = [UIButton buttonWithType:UIButtonTypeRoundedRect];
  [button1 setTitle:[NSString stringWithUTF8String:"CALayer tests"]
           forState:UIControlStateNormal];
  [button1 setFrame:CGRectMake(40, 300, 240, 40)];
  [button1 addTarget:self
                action:@selector(goToCALayerTests)
      forControlEvents:UIControlEventTouchUpInside];

  [self addSubview:button1];

  return self;
}

- (void)dealloc {
  [timer invalidate];
  [timer release];
  [ball release];
  [super dealloc];
}

- (void)moveBall:(NSTimer *)timer {
  CGRect windowFrame = [self bounds];
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

- (void)goToCALayerTests {
  // break the strong reference cycle
  [timer invalidate];
  [timer release];
  timer = nil;

  [((GUITestsAppDelegate *)[[UIApplication sharedApplication]
      delegate]) setMainView:[[[GUITestsCALayerTestsView alloc]
                                 initWithFrame:[self frame]] autorelease]];
}

@end
