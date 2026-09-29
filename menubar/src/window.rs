//! The settings window: what the app shows on a normal launch.
//!
//! Flat coordinates, no auto-layout: the window is a fixed list of controls
//! laid out top to bottom, so a y cursor is less machinery than an
//! `NSStackView` and its constraints.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSFont, NSPopUpButton, NSTextField, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::i18n::{Lang, Strings};
use crate::settings::{TitleStyle, DEFAULT_PRESETS};
use crate::{
    TAG_SET_DOCK_ICON, TAG_SET_LAUNCH_LOGIN, TAG_SET_START_MINIMIZED, TAG_SET_STATUS_ITEM,
};

/// Window size, and how the rows are spaced.
const WIDTH: f64 = 470.0;
const HEIGHT: f64 = 486.0;
const MARGIN: f64 = 20.0;
const ROW_HEIGHT: f64 = 24.0;
const ROW_GAP: f64 = 8.0;
const SECTION_GAP: f64 = 16.0;

/// The settings window and the controls inside it, so the caller can seed and
/// re-read them without walking the view tree.
pub struct SettingsWindow {
    /// The window itself.
    pub window: Retained<NSWindow>,
    /// The four checkboxes, in display order.
    pub checks: Vec<Retained<NSButton>>,
    /// The three preset speed fields.
    pub preset_fields: Vec<Retained<NSTextField>>,
    /// Menu bar title options, in `TitleStyle::all()` order.
    pub style_buttons: Vec<Retained<NSButton>>,
    /// Language picker: system first, then `Lang::all()`.
    pub language_popup: Retained<NSPopUpButton>,
}

/// Build the settings window. Controls send their actions to `target`.
pub fn build<T: NSObjectProtocol + 'static>(
    mtm: MainThreadMarker,
    s: &'static Strings,
    version: &str,
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

    // Rows are laid out from the top down with a cursor.
    let mut y = HEIGHT - MARGIN - ROW_HEIGHT;
    let label = |text: &str, y: f64, size: f64| {
        let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        field.setFrame(NSRect::new(
            NSPoint::new(MARGIN, y),
            NSSize::new(WIDTH - 2.0 * MARGIN, ROW_HEIGHT),
        ));
        field.setFont(Some(&NSFont::systemFontOfSize(size)));
        content.addSubview(&field);
        field
    };
    let checkbox = |text: &str, tag: isize, y: f64| {
        let button = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &NSString::from_str(text),
                Some(target.as_any_object()),
                Some(sel!(toggleSetting:)),
                mtm,
            )
        };
        button.setFrame(NSRect::new(
            NSPoint::new(MARGIN, y),
            NSSize::new(WIDTH - 2.0 * MARGIN, ROW_HEIGHT),
        ));
        button.setTag(tag);
        content.addSubview(&button);
        button
    };

    let heading = label(&format!("macfan {version}"), y, 13.0);
    heading.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));

    let mut checks = Vec::with_capacity(4);
    let rows: [(isize, &str); 4] = [
        (TAG_SET_STATUS_ITEM, s.show_status_item),
        (TAG_SET_DOCK_ICON, s.show_dock_icon),
        (TAG_SET_LAUNCH_LOGIN, s.launch_at_login),
        (TAG_SET_START_MINIMIZED, s.start_minimized),
    ];
    for (tag, text) in rows {
        y -= ROW_HEIGHT + ROW_GAP;
        checks.push(checkbox(text, tag, y));
    }

    y -= ROW_HEIGHT + ROW_GAP;
    let note = label(s.keep_one_visible, y, 11.0);
    let _ = note;

    // --- preset speeds -----------------------------------------------------
    y -= ROW_HEIGHT + SECTION_GAP;
    label(s.presets_label, y, 12.0);
    y -= ROW_HEIGHT + ROW_GAP;
    let field_width = 66.0;
    let field_step = field_width + 10.0;
    let mut preset_fields = Vec::with_capacity(3);
    for (index, rpm) in DEFAULT_PRESETS.iter().enumerate() {
        let field = NSTextField::new(mtm);
        field.setFrame(NSRect::new(
            NSPoint::new(MARGIN + index as f64 * field_step, y),
            NSSize::new(field_width, ROW_HEIGHT),
        ));
        field.setStringValue(&NSString::from_str(&rpm.to_string()));
        content.addSubview(&field);
        preset_fields.push(field);
    }
    let apply_button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(s.apply),
            Some(target.as_any_object()),
            Some(sel!(applyPresets:)),
            mtm,
        )
    };
    apply_button.setFrame(NSRect::new(
        NSPoint::new(MARGIN + 3.0 * field_step + 6.0, y),
        NSSize::new(90.0, ROW_HEIGHT),
    ));
    content.addSubview(&apply_button);

    // --- menu bar title style ---------------------------------------------
    y -= ROW_HEIGHT + SECTION_GAP;
    label(s.title_style_label, y, 12.0);
    y -= ROW_HEIGHT + ROW_GAP;
    let style_titles = [s.style_icon_rpm, s.style_rpm_only, s.style_icon_temp];
    let mut style_buttons = Vec::with_capacity(3);
    let mut x = MARGIN;
    for (index, style) in TitleStyle::all().iter().enumerate() {
        let title = &style_titles[index];
        let _ = style;
        let button = unsafe {
            NSButton::radioButtonWithTitle_target_action(
                &NSString::from_str(title),
                Some(target.as_any_object()),
                Some(sel!(titleStyleChanged:)),
                mtm,
            )
        };
        let text_width = title.chars().count() as f64 * 8.0 + 40.0;
        button.setFrame(NSRect::new(
            NSPoint::new(x, y),
            NSSize::new(text_width, ROW_HEIGHT),
        ));
        button.setTag(index as isize);
        content.addSubview(&button);
        style_buttons.push(button);
        x += text_width + 6.0;
    }

    // --- language ----------------------------------------------------------
    y -= ROW_HEIGHT + SECTION_GAP;
    label(s.language_label, y, 12.0);
    y -= ROW_HEIGHT + ROW_GAP;
    let language_popup = NSPopUpButton::new(mtm);
    language_popup.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(190.0, ROW_HEIGHT),
    ));
    language_popup.addItemWithTitle(&NSString::from_str(s.language_system));
    for lang in Lang::all() {
        language_popup.addItemWithTitle(&NSString::from_str(language_name(lang)));
    }
    unsafe {
        language_popup.setTarget(Some(target.as_any_object()));
        language_popup.setAction(Some(sel!(languageChanged:)));
    }
    content.addSubview(&language_popup);

    window.setContentView(Some(&content));
    window.center();
    SettingsWindow {
        window,
        checks,
        preset_fields,
        style_buttons,
        language_popup,
    }
}

/// Endonym for a language, so the list reads the same in every UI language.
pub fn language_name(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "English",
        Lang::ZhHans => "简体中文",
        Lang::ZhHant => "繁體中文",
        Lang::Ja => "日本語",
    }
}

/// Tag of the language popup entry at `index` (0 = follow the system).
pub fn language_tag(index: usize) -> Option<Option<&'static str>> {
    let all = Lang::all();
    if index == 0 {
        Some(None)
    } else {
        all.get(index - 1).map(|lang| Some(lang.code()))
    }
}

/// Handy for `&dyn AnyObject`-ish call sites.
trait AsAnyObject {
    fn as_any_object(&self) -> &AnyObject;
}

impl<T: NSObjectProtocol> AsAnyObject for T {
    fn as_any_object(&self) -> &AnyObject {
        // The object lives as long as the app does, so the borrow is valid for
        // the lifetime of the controls.
        unsafe { &*(self as *const T as *const AnyObject) }
    }
}
