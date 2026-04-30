#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

@interface NSPasteboard (Yarp)
- (NSArray *)getFilePaths;
@end

/// YarpHostView is the Content view of a Yarp window.
// It is backed by a Metal CALayer.
@interface YarpHostView : NSView <CALayerDelegate, NSTextInputClient>
- (YarpHostView *)initWithFrame:(NSRect)frame
                    metalDevice:(id)metalDevice
             enableTitlebarDrag:(BOOL)enableTitlebarDrag
                       testMode:(BOOL)testMode;
- (void)setAsyncCallback:(BOOL)shouldAsync;
- (BOOL)keyDownImpl:(NSEvent *)event;
@end
