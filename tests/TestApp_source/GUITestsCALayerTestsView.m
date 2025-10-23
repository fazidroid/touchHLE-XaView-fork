/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#include "system_headers.h"

#include "GUITestsCALayerTestsView.h"

#define NUM_TESTS 3

@implementation GUITestsCALayerTestsView : UIView

UILabel *label;
UIView *testArea;
NSUInteger testNum;

- (instancetype)initWithFrame:(CGRect)frame {
  [super initWithFrame:frame];

  label = [[UILabel alloc] initWithFrame:[self bounds]];
  label.text = [NSString stringWithUTF8String:"CALayer tests (press →)"];
  label.textAlignment = UITextAlignmentCenter;
  label.frame = CGRectMake(0, 0, 320, 20);
  [self addSubview:label];

  UIButton *button1 = [UIButton buttonWithType:UIButtonTypeRoundedRect];
  [button1 setTitle:[NSString stringWithUTF8String:"←"]
           forState:UIControlStateNormal];
  [button1 setFrame:CGRectMake(0, 420, 40, 40)];
  [button1 addTarget:self
                action:@selector(prevTest)
      forControlEvents:UIControlEventTouchUpInside];
  [self addSubview:button1];
  [button1 layoutSubviews]; // FIXME: workaround for touchHLE not calling this

  UIButton *button2 = [UIButton buttonWithType:UIButtonTypeRoundedRect];
  [button2 setTitle:[NSString stringWithUTF8String:"→"]
           forState:UIControlStateNormal];
  [button2 setFrame:CGRectMake(280, 420, 40, 40)];
  [button2 addTarget:self
                action:@selector(nextTest)
      forControlEvents:UIControlEventTouchUpInside];
  [self addSubview:button2];
  [button2 layoutSubviews]; // FIXME: workaround for touchHLE not calling this

  // Don't display any test initially. The testing for convertPoint:toLayer: etc
  // won't produce the right results until this view has actually been added to
  // the window.
  testNum = 0;

  return self;
}

- (void)dealloc {
  [label release];
  [testArea release];
  [super dealloc];
}

- (void)prevTest {
  if (testNum > 1)
    testNum--;
  [self displayTest];
}
- (void)nextTest {
  if (testNum < NUM_TESTS)
    testNum++;
  [self displayTest];
}
- (void)displayTest {
  label.text = [NSString
      stringWithFormat:[NSString stringWithUTF8String:"CALayer test %u/%u"],
                       testNum, NUM_TESTS];
  [testArea removeFromSuperview];
  [testArea release];
  testArea = [[UIView alloc] initWithFrame:CGRectMake(10, 30, 300, 300)];
  testArea.backgroundColor = [UIColor grayColor];
  [self addSubview:testArea];

  [self performSelector:NSSelectorFromString([NSString
                            stringWithFormat:[NSString
                                                 stringWithUTF8String:"test%u"],
                                             testNum])];
}

- (UIView *)addViewWithFrame:(CGRect)frame color:(UIColor *)color {
  UIView *view = [[UIView alloc] initWithFrame:frame];
  view.backgroundColor = color;
  [testArea addSubview:view];
  [view release];
  return view;
}
- (UILabel *)addLabelWithFrame:(CGRect)frame text:(NSString *)text {
  UILabel *label = [[UILabel alloc] initWithFrame:frame];
  label.text = text;
  label.textColor = [UIColor whiteColor];
  label.backgroundColor = [UIColor clearColor];
  [testArea addSubview:label];
  [label release];
  return label;
}

// These tests should all look like three squares arranged diagonally.
// The color differences make it more obvious when you've switched tests.

- (void)test1 {
  UIView *view1 = [self addViewWithFrame:CGRectMake(0, 0, 100, 100)
                                   color:[UIColor redColor]];
  [self addLabelWithFrame:CGRectMake(0, 0, 100, 25)
                     text:NSStringFromCGPoint([view1.layer
                              convertPoint:CGPointMake(0.0, 0.0)
                                 fromLayer:nil])];
  [self addLabelWithFrame:CGRectMake(0, 25, 100, 25)
                     text:NSStringFromCGPoint([view1
                              convertPoint:CGPointMake(0.0, 0.0)
                                  fromView:nil])];
  [self addLabelWithFrame:CGRectMake(0, 50, 100, 25)
                     text:NSStringFromCGPoint([view1.layer
                              convertPoint:CGPointMake(0.0, 0.0)
                                   toLayer:view1.window.layer])];
  [self addLabelWithFrame:CGRectMake(0, 75, 100, 25)
                     text:NSStringFromCGPoint([view1.window
                              convertPoint:CGPointMake(0.0, 0.0)
                                  toWindow:nil])];
  [self addViewWithFrame:CGRectMake(100, 100, 100, 100)
                   color:[UIColor greenColor]];
  [self addViewWithFrame:CGRectMake(200, 200, 100, 100)
                   color:[UIColor blueColor]];
}
- (void)test2 {
  UIView *view1 = [self addViewWithFrame:CGRectMake(0, 0, 100, 100)
                                   color:[UIColor cyanColor]];
  view1.layer.anchorPoint = CGPointMake(0.0, 0.0);
  view1.layer.position = CGPointMake(0.0, 0.0);
  UIView *view2 = [self addViewWithFrame:CGRectMake(0, 0, 100, 100)
                                   color:[UIColor magentaColor]];
  view2.layer.anchorPoint = CGPointMake(0.5, 0.5);
  view2.layer.position = CGPointMake(150.0, 150.0);
  UIView *view3 = [self addViewWithFrame:CGRectMake(0, 0, 100, 100)
                                   color:[UIColor yellowColor]];
  view3.layer.anchorPoint = CGPointMake(1.0, 1.0);
  view3.layer.position = CGPointMake(300.0, 300.0);
}
- (void)test3 {
  UIView *view1 = [self addViewWithFrame:CGRectMake(0, 0, 100, 100)
                                   color:[UIColor orangeColor]];
  view1.layer.affineTransform = CGAffineTransformMakeTranslation(0.0, 0.0);
  UIView *view2 = [self addViewWithFrame:CGRectMake(0, 0, 100, 100)
                                   color:[UIColor greenColor]];
  view2.layer.affineTransform = CGAffineTransformMakeTranslation(100.0, 100.0);
  UIView *view3 = [self addViewWithFrame:CGRectMake(0, 0, 100, 100)
                                   color:[UIColor purpleColor]];
  view3.layer.anchorPoint = CGPointMake(1.0, 1.0);
  view3.layer.position = CGPointMake(200.0, 200.0);
  view3.layer.affineTransform = CGAffineTransformMakeTranslation(100.0, 100.0);
}

@end
