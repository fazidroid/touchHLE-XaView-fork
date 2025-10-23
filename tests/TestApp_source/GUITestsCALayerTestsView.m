/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#include "system_headers.h"

#include "GUITestsCALayerTestsView.h"

@implementation GUITestsCALayerTestsView : UIView

- (instancetype)initWithFrame:(CGRect)frame {
  [super initWithFrame:frame];

  UILabel *label = [[[UILabel alloc] initWithFrame:[self bounds]] autorelease];
  label.text = [NSString stringWithUTF8String:"TODO"];
  label.textAlignment = UITextAlignmentCenter;
  [self addSubview:label];

  return self;
}

@end
