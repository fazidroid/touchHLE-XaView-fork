/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

// We do not have complete system headers for iPhone OS, so we must declare some
// things ourselves rather than #include'ing.
#include "system_headers_objc.h"
#include <stdio.h>

typedef float CGFloat;
typedef struct {
  CGFloat x, y;
} CGPoint;
typedef struct {
  CGFloat width, height;
} CGSize;
typedef struct {
  CGPoint origin;
  CGSize size;
} CGRect;

typedef enum {
  UITextAlignmentLeft = 0,
  UITextAlignmentCenter = 1,
  UITextAlignmentRight = 2,
} UITextAlignment;

@interface NSString : NSObject
+ (instancetype)stringWithUTF8String:(const char *)string;
@end
@interface UIScreen : NSObject
+ (instancetype)mainScreen;
- (CGRect)applicationFrame;
@end
@interface UIView : NSObject
- (instancetype)initWithFrame:(CGRect)frame;
- (CGRect)bounds;
- (void)addSubview:(UIView *)view;
@end
@interface UIWindow : UIView
- (void)makeKeyAndVisible;
@end
@interface UILabel : UIView
- (void)setText:(NSString *)text;
- (void)setTextAlignment:(UITextAlignment)alignment;
@end

NSString *NSStringFromClass(Class);
int UIApplicationMain(int, char **, NSString *, NSString *);

@interface TestAppDelegate : NSObject
@end
@implementation TestAppDelegate : NSObject
- (void)applicationDidFinishLaunching:(id)app {
  UIWindow *window =
      [[UIWindow alloc] initWithFrame:[[UIScreen mainScreen] applicationFrame]];
  UILabel *label = [[UILabel alloc] initWithFrame:[window bounds]];
  label.text = [NSString stringWithUTF8String:"hello, world! 🌏"];
  label.textAlignment = UITextAlignmentCenter;
  [window addSubview:label];
  [window makeKeyAndVisible];
}
@end

int TestApp_gui_tests_main(int argc, char **argv) {
  id pool = [NSAutoreleasePool new];
  int res = UIApplicationMain(argc, argv, NULL,
                              NSStringFromClass([TestAppDelegate class]));
  [pool release];
  return res;
}
