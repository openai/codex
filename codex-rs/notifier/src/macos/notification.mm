#include <AppKit/AppKit.h>
#include <UserNotifications/UserNotifications.h>
#include <dispatch/dispatch.h>
#include <stdio.h>
#include <stdlib.h>

@interface CodexNotificationDelegate : NSObject <UNUserNotificationCenterDelegate>
@end

@implementation CodexNotificationDelegate
- (void)userNotificationCenter:(UNUserNotificationCenter *)center
        willPresentNotification:(UNNotification *)notification
          withCompletionHandler:(void (^)(UNNotificationPresentationOptions options))completionHandler {
    completionHandler(UNNotificationPresentationOptionBanner | UNNotificationPresentationOptionSound);
}

- (void)userNotificationCenter:(UNUserNotificationCenter *)center
 didReceiveNotificationResponse:(UNNotificationResponse *)response
          withCompletionHandler:(void (^)(void))completionHandler {
    completionHandler();
}
@end

static void install_delegate(UNUserNotificationCenter *center) {
    static dispatch_once_t onceToken;
    static CodexNotificationDelegate *delegate = nil;
    dispatch_once(&onceToken, ^{
        delegate = [CodexNotificationDelegate new];
        center.delegate = delegate;
    });
}

static BOOL ensure_authorization(UNUserNotificationCenter *center) {
    dispatch_semaphore_t settings_wait = dispatch_semaphore_create(0);
    __block UNAuthorizationStatus status = UNAuthorizationStatusNotDetermined;

    [center
        getNotificationSettingsWithCompletionHandler:^(UNNotificationSettings *settings) {
          status = settings.authorizationStatus;
          dispatch_semaphore_signal(settings_wait);
        }];

    dispatch_semaphore_wait(settings_wait, DISPATCH_TIME_FOREVER);

    if (status == UNAuthorizationStatusDenied) {
        fprintf(stderr, "[codex-notifier] notification authorization currently denied\n");
        return NO;
    }

    if (status == UNAuthorizationStatusAuthorized ||
        status == UNAuthorizationStatusProvisional) {
        return YES;
    }

    dispatch_semaphore_t request_wait = dispatch_semaphore_create(0);
    __block BOOL granted = NO;

    [center requestAuthorizationWithOptions:(UNAuthorizationOptionAlert | UNAuthorizationOptionSound |
                                             UNAuthorizationOptionBadge)
                          completionHandler:^(BOOL success, NSError *error) {
                            granted = success;
                            if (error) {
                                fprintf(stderr, "[codex-notifier] authorization request error: %s\n",
                                        error.localizedDescription.UTF8String);
                            }
                            dispatch_semaphore_signal(request_wait);
                          }];

    dispatch_semaphore_wait(request_wait, DISPATCH_TIME_FOREVER);
    return granted;
}

extern "C" int codex_post_user_notification(const char *title_c,
                                            const char *subtitle_c,
                                            const char *body_c,
                                            const char *icon_path_c) {
    setenv("__CFBundleIdentifier", "com.openai.codex.notifier", 0);

    @autoreleasepool {
        UNUserNotificationCenter *center = [UNUserNotificationCenter currentNotificationCenter];
        if (!center) {
            fprintf(stderr, "[codex-notifier] UNUserNotificationCenter unavailable\n");
            return -10;
        }

        install_delegate(center);

        if (!ensure_authorization(center)) {
            return -11;
        }

        UNMutableNotificationContent *content = [[UNMutableNotificationContent alloc] init];
        if (!content) {
            fprintf(stderr, "[codex-notifier] failed to allocate notification content\n");
            return -12;
        }

        if (title_c && title_c[0] != '\0') {
            content.title = [NSString stringWithUTF8String:title_c];
        } else {
            content.title = @"Codex CLI";
        }

        if (subtitle_c && subtitle_c[0] != '\0') {
            content.subtitle = [NSString stringWithUTF8String:subtitle_c];
        }

        if (body_c && body_c[0] != '\0') {
            content.body = [NSString stringWithUTF8String:body_c];
        }

        if (icon_path_c && icon_path_c[0] != '\0') {
            NSError *attachmentError = nil;
            NSString *path = [NSString stringWithUTF8String:icon_path_c];
            UNNotificationAttachment *attachment =
                [UNNotificationAttachment attachmentWithIdentifier:@"codex-icon"
                                                               URL:[NSURL fileURLWithPath:path]
                                                           options:nil
                                                             error:&attachmentError];
            if (attachment) {
                content.attachments = @[attachment];
            } else if (attachmentError) {
                fprintf(stderr, "[codex-notifier] attachment error: %s\n",
                        attachmentError.localizedDescription.UTF8String);
            }
        }

        UNTimeIntervalNotificationTrigger *trigger =
            [UNTimeIntervalNotificationTrigger triggerWithTimeInterval:0.1 repeats:NO];
        NSString *identifier =
            [NSString stringWithFormat:@"com.openai.codex.notifier.%@", [[NSUUID UUID] UUIDString]];
        UNNotificationRequest *request =
            [UNNotificationRequest requestWithIdentifier:identifier content:content trigger:trigger];

        dispatch_semaphore_t submit_wait = dispatch_semaphore_create(0);
        __block NSError *submit_error = nil;
        [center addNotificationRequest:request
                  withCompletionHandler:^(NSError *error) {
                      submit_error = error;
                      dispatch_semaphore_signal(submit_wait);
                  }];

        dispatch_semaphore_wait(submit_wait, DISPATCH_TIME_FOREVER);

        if (submit_error) {
            fprintf(stderr, "[codex-notifier] addNotificationRequest error: %s\n",
                    submit_error.localizedDescription.UTF8String);
            return -13;
        }

        return 0;
    }
}
