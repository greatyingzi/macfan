//! The settings window: what the app shows on a normal launch.
//!
//! Flat coordinates, no auto-layout: this window is a fixed list of four
//! checkboxes and a note, so laying it out by hand is less machinery than an
//! `NSStackView` and its constraints.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSFont, NSTextField, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::i18n::Strings;
use crate::{
    TAG_SET_DOCK_ICON, TAG_SET_LAUNCH_LOGIN, TAG_SET_START_MINIMIZED, TAG_SET_STATUS_ITEM,
};

/// Window size, and where the controls sit inside it.
const WIDTH: f64 = 420.0;
const HEIGHT: f64 = 232.0;
const MARGIN: f64 = 20.0;
const ROW_HEIGHT: f64 = 24.0;
const ROW_GAP: f64 = 10.0;

/// The settings window and the checkboxes inside it (so the caller can keep
/// their ticks in sync without walking the view tree).
pub struct SettingsWindow {
    /// The window itself.
    pub window: Retained<NSWindow>,
    /// The four checkboxes, in display order.
    pub checks: Vec<Retained<NSButton>>,
}

/// Build the settings window. The checkboxes send `toggleSetting:` to `target`
/// and carry a tag identifying which setting they belong to.
pub fn build<T: NSObjectProtocol + 'static>(
    mtm: MainThreadMarker,
    s: &'static Strings,
    target: &T,
) -> SettingsWindow {
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str(s.settings_title));
    // Closing hides the window instead of releasing it.
    unsafe { window.setReleasedWhenClosed(false) };

    let content: Retained<NSView> = NSView::new(mtm);
    content.setFrame(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(WIDTH, HEIGHT),
    ));

    let mut y = HEIGHT - MARGIN - ROW_HEIGHT;
    let heading = NSTextField::labelWithString(&NSString::from_str("macfan"), mtm);
    heading.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(WIDTH - 2.0 * MARGIN, ROW_HEIGHT),
    ));
    heading.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
    content.addSubview(&heading);

    let mut checks = Vec::with_capacity(4);
    let rows: [(isize, &str); 4] = [
        (TAG_SET_STATUS_ITEM, s.show_status_item),
        (TAG_SET_DOCK_ICON, s.show_dock_icon),
        (TAG_SET_LAUNCH_LOGIN, s.launch_at_login),
        (TAG_SET_START_MINIMIZED, s.start_minimized),
    ];
    for (tag, title) in rows {
        y -= ROW_HEIGHT + ROW_GAP;
        let checkbox = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &NSString::from_str(title),
                Some(target.as_any_object()),
                Some(sel!(toggleSetting:)),
                mtm,
            )
        };
        checkbox.setFrame(NSRect::new(
            NSPoint::new(MARGIN, y),
            NSSize::new(WIDTH - 2.0 * MARGIN, ROW_HEIGHT),
        ));
        checkbox.setTag(tag);
        content.addSubview(&checkbox);
        checks.push(checkbox);
    }

    y -= ROW_HEIGHT + ROW_GAP;
    let note = NSTextField::labelWithString(&NSString::from_str(s.keep_one_visible), mtm);
    note.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(WIDTH - 2.0 * MARGIN, ROW_HEIGHT),
    ));
    note.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    content.addSubview(&note);

    window.setContentView(Some(&content));
    window.center();
    SettingsWindow { window, checks }
}

/// Handy for `&dyn AnyObject`-ish call sites.
trait AsAnyObject {
    fn as_any_object(&self) -> &AnyObject;
}

impl<T: NSObjectProtocol> AsAnyObject for T {
    fn as_any_object(&self) -> &AnyObject {
        // The object lives in the window for as long as the app does, so the
        // borrow is valid for the lifetime of the checkboxes.
        unsafe { &*(self as *const T as *const AnyObject) }
    }
}
