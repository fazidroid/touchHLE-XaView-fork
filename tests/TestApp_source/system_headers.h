/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#ifndef TOUCHHLE_SYSTEM_HEADERS_H
#define TOUCHHLE_SYSTEM_HEADERS_H

// This file contains definitions of types etc we don't have in our SDK, which
// is built from open-source headers.

#include <CoreFoundation/CFData.h>
#include <stdbool.h>
#include <stddef.h>

// Objective-C runtime

typedef signed char BOOL;
#define YES 0
#define NO 1

typedef unsigned long NSUInteger;
typedef signed long NSInteger;

// id objc_msgSend(id, SEL, ...);

// Foundation

@interface NSObject {
  Class isa;
}
+ (Class)class;
+ (instancetype)alloc;
+ (instancetype)new;
- (instancetype)init;
- (instancetype)retain;
- (void)release;
- (instancetype)autorelease;
- (void)dealloc;
- (NSUInteger)retainCount;
@end

@interface NSAutoreleasePool : NSObject
+ (void)addObject:(id)anObject;
- (void)addObject:(id)anObject;
- (void)drain;
@end

@interface NSString : NSObject
+ (instancetype)stringWithUTF8String:(const char *)string;
@end

NSString *NSStringFromClass(Class);

// Core Graphics

// (See CGAffineTransform.c for where this define comes from.)
#ifdef DEFINE_ME_WHEN_BUILDING_ON_MACOS
typedef double CGFloat; // 64-bit definition (not supported by touchHLE)
#else
typedef float CGFloat;
#endif

typedef struct {
  CGFloat x, y;
} CGPoint;
bool CGPointEqualToPoint(CGPoint, CGPoint);
typedef struct {
  CGFloat width, height;
} CGSize;
bool CGSizeEqualToSize(CGSize, CGSize);
typedef struct {
  CGPoint origin;
  CGSize size;
} CGRect;
bool CGRectEqualToRect(CGRect, CGRect);

typedef struct {
  CGFloat a, b, c, d, tx, ty;
} CGAffineTransform;
// extern const CGAffineTransform CGAffineTransformIdentity;
bool CGAffineTransformIsIdentity(CGAffineTransform);
bool CGAffineTransformEqualToTransform(CGAffineTransform, CGAffineTransform);
CGAffineTransform CGAffineTransformMake(CGFloat, CGFloat, CGFloat, CGFloat,
                                        CGFloat, CGFloat);
CGAffineTransform CGAffineTransformMakeRotation(CGFloat);
CGAffineTransform CGAffineTransformMakeScale(CGFloat, CGFloat);
CGAffineTransform CGAffineTransformMakeTranslation(CGFloat, CGFloat);
CGAffineTransform CGAffineTransformConcat(CGAffineTransform, CGAffineTransform);
CGAffineTransform CGAffineTransformRotate(CGAffineTransform, CGFloat);
CGAffineTransform CGAffineTransformScale(CGAffineTransform, CGFloat, CGFloat);
CGAffineTransform CGAffineTransformTranslate(CGAffineTransform, CGFloat,
                                             CGFloat);
CGAffineTransform CGAffineTransformInvert(CGAffineTransform);
CGPoint CGPointApplyAffineTransform(CGPoint, CGAffineTransform);
CGSize CGSizeApplyAffineTransform(CGSize, CGAffineTransform);
CGRect CGRectApplyAffineTransform(CGRect, CGAffineTransform);

// `CGDataProvider.h`

typedef struct _CGDataProvider *CGDataProviderRef;

CGDataProviderRef CGDataProviderCreateWithCFData(CFDataRef);
CFDataRef CGDataProviderCopyData(CGDataProviderRef);

// `CGGeometry.h`

CGFloat CGRectGetMinX(CGRect);
CGFloat CGRectGetMaxX(CGRect);
CGFloat CGRectGetMinY(CGRect);
CGFloat CGRectGetMaxY(CGRect);
CGFloat CGRectGetHeight(CGRect);
CGFloat CGRectGetWidth(CGRect);

// `CGImage.h`

typedef struct _CGImage *CGImageRef;

CGImageRef CGImageCreateWithJPEGDataProvider(CGDataProviderRef, const CGFloat *,
                                             bool, int);
size_t CGImageGetWidth(CGImageRef);
size_t CGImageGetHeight(CGImageRef);
CGDataProviderRef CGImageGetDataProvider(CGImageRef);

// UIKit

typedef enum {
  UITextAlignmentLeft = 0,
  UITextAlignmentCenter = 1,
  UITextAlignmentRight = 2,
} UITextAlignment;

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

int UIApplicationMain(int, char **, NSString *, NSString *);

#endif // TOUCHHLE_SYSTEM_HEADERS_H
