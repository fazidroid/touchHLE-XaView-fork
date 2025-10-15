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
