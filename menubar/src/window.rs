//! The settings window.
//!
//! Frosted background (an `NSVisualEffectView`, the same material macOS uses
//! for sidebars), sections with muted headers, and popups for the mutually
//! exclusive choices — a popup cannot truncate the way a row of radio buttons
//! did, and it reads as "pick one" without extra explanation.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSColor, NSFont, NSFontWeightMedium, NSPopUpButton, NSTextField,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::i18n::{Lang, Strings};
use crate::settings::{TitleContent, DEFAULT_PRESETS};
use crate::{
    TAG_SET_DOCK_ICON, TAG_SET_LAUNCH_LOGIN, TAG_SET_START_MINIMIZED, TAG_SET_STATUS_ITEM,
    TAG_SET_TITLE_ICON,
};

const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 560.0; // one more row than the three-style layout had
const MARGIN: f64 = 22.0;
const ROW_HEIGHT: f64 = 24.0;
const ROW_GAP: f64 = 6.0;
const SECTION_GAP: f64 = 18.0;
const LABEL_WIDTH: f64 = 96.0;

/// The settings window and the controls inside it, so the caller can seed and
/// re-read them without walking the view tree.
pub struct SettingsWindow {
    /// The window itself.
    pub window: Retained<NSWindow>,
    /// The five checkboxes, in display order.
    pub checks: Vec<Retained<NSButton>>,
    /// The three preset speed fields.
    pub preset_fields: Vec<Retained<NSTextField>>,
    /// What the title reports: entries follow `TitleContent::all()`.
    pub content_popup: Retained<NSPopUpButton>,
    /// Language picker: system first, then `Lang::all()`.
    pub language_popup: Retained<NSPopUpButton>,
    /// Live line under the heading: what the fans are doing right now.
    pub subtitle: Retained<NSTextField>,
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
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::FullSizeContentView,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str(s.settings_title));
    // The blur runs under the title bar, so the title bar itself goes away.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitlebarAppearsTransparent(true);

    let blur = NSVisualEffectView::new(mtm);
    blur.setFrame(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(WIDTH, HEIGHT),
    ));
    blur.setMaterial(NSVisualEffectMaterial::Sidebar);
    blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    blur.setState(NSVisualEffectState::FollowsWindowActiveState);

    let mut y = HEIGHT - MARGIN - 34.0;

    // --- heading and live state -------------------------------------------
    let heading =
        NSTextField::labelWithString(&NSString::from_str(&format!("macfan {version}")), mtm);
    heading.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(WIDTH - 2.0 * MARGIN, 26.0),
    ));
    heading.setFont(Some(&*NSFont::boldSystemFontOfSize(17.0)));
    blur.addSubview(&heading);

    y -= 24.0;
    let subtitle = NSTextField::labelWithString(&NSString::from_str("—"), mtm);
    subtitle.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(WIDTH - 2.0 * MARGIN, 20.0),
    ));
    subtitle.setFont(Some(&*NSFont::systemFontOfSize(12.0)));
    subtitle.setTextColor(Some(&*NSColor::secondaryLabelColor()));
    blur.addSubview(&subtitle);

    // --- helpers ----------------------------------------------------------
    let header = |text: &str, y: f64| {
        let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        field.setFrame(NSRect::new(
            NSPoint::new(MARGIN, y),
            NSSize::new(WIDTH - 2.0 * MARGIN, 18.0),
        ));
        field.setFont(Some(&*NSFont::systemFontOfSize_weight(11.0, unsafe {
            NSFontWeightMedium
        })));
        field.setTextColor(Some(&*NSColor::secondaryLabelColor()));
        blur.addSubview(&field);
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
        blur.addSubview(&button);
        button
    };
    let popup = |items: &[&str], y: f64| {
        let button = NSPopUpButton::new(mtm);
        button.setFrame(NSRect::new(
            NSPoint::new(MARGIN + LABEL_WIDTH, y),
            NSSize::new(WIDTH - 2.0 * MARGIN - LABEL_WIDTH, ROW_HEIGHT),
        ));
        for item in items {
            button.addItemWithTitle(&NSString::from_str(item));
        }
        blur.addSubview(&button);
        button
    };

    // --- menu bar ---------------------------------------------------------
    y -= SECTION_GAP;
    header(s.section_menu_bar, y);
    y -= ROW_HEIGHT + ROW_GAP;
    let mut checks = Vec::with_capacity(5);
    checks.push(checkbox(s.show_status_item, TAG_SET_STATUS_ITEM, y));

    // Two independent choices, not one list that mixes them: what the number
    // is, and whether a glyph sits beside it.
    y -= ROW_HEIGHT + ROW_GAP;
    let content_label =
        NSTextField::labelWithString(&NSString::from_str(s.title_content_label), mtm);
    content_label.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(LABEL_WIDTH, ROW_HEIGHT),
    ));
    blur.addSubview(&content_label);
    let content_popup = popup(&[s.content_speed, s.content_temp], y);
    unsafe {
        content_popup.setTarget(Some(target.as_any_object()));
        content_popup.setAction(Some(sel!(titleContentChanged:)));
    }

    y -= ROW_HEIGHT + ROW_GAP;
    checks.push(checkbox(s.title_icon_label, TAG_SET_TITLE_ICON, y));

    // --- application ------------------------------------------------------
    y -= ROW_HEIGHT + SECTION_GAP;
    header(s.section_app, y);
    y -= ROW_HEIGHT + ROW_GAP;
    checks.push(checkbox(s.show_dock_icon, TAG_SET_DOCK_ICON, y));
    y -= ROW_HEIGHT + ROW_GAP;
    checks.push(checkbox(s.launch_at_login, TAG_SET_LAUNCH_LOGIN, y));
    y -= ROW_HEIGHT + ROW_GAP;
    checks.push(checkbox(s.start_minimized, TAG_SET_START_MINIMIZED, y));

    // --- presets ----------------------------------------------------------
    y -= ROW_HEIGHT + SECTION_GAP;
    header(s.presets_label, y);
    y -= ROW_HEIGHT + ROW_GAP;
    let field_width = 70.0;
    let field_step = field_width + 8.0;
    let mut preset_fields = Vec::with_capacity(3);
    for (index, rpm) in DEFAULT_PRESETS.iter().enumerate() {
        let field = NSTextField::new(mtm);
        field.setFrame(NSRect::new(
            NSPoint::new(MARGIN + index as f64 * field_step, y),
            NSSize::new(field_width, ROW_HEIGHT),
        ));
        field.setStringValue(&NSString::from_str(&rpm.to_string()));
        blur.addSubview(&field);
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
        NSSize::new(96.0, ROW_HEIGHT + 2.0),
    ));
    // The one coloured control: it commits, so it carries the accent.
    apply_button.setBezelColor(Some(&*NSColor::controlAccentColor()));
    blur.addSubview(&apply_button);

    // --- language ---------------------------------------------------------
    y -= ROW_HEIGHT + SECTION_GAP;
    header(s.language_label, y);
    y -= ROW_HEIGHT + ROW_GAP;
    let language_label = NSTextField::labelWithString(&NSString::from_str(s.language_label), mtm);
    language_label.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(LABEL_WIDTH, ROW_HEIGHT),
    ));
    language_label.setHidden(true);
    blur.addSubview(&language_label);
    let mut language_items = vec![s.language_system];
    for lang in Lang::all() {
        language_items.push(language_name(lang));
    }
    let language_popup = popup(&language_items, y);
    unsafe {
        language_popup.setTarget(Some(target.as_any_object()));
        language_popup.setAction(Some(sel!(languageChanged:)));
    }

    // --- footer -----------------------------------------------------------
    y -= ROW_HEIGHT + SECTION_GAP;
    let note = NSTextField::labelWithString(&NSString::from_str(s.keep_one_visible), mtm);
    note.setFrame(NSRect::new(
        NSPoint::new(MARGIN, y),
        NSSize::new(WIDTH - 2.0 * MARGIN, 32.0),
    ));
    note.setFont(Some(&*NSFont::systemFontOfSize(11.0)));
    note.setTextColor(Some(&*NSColor::tertiaryLabelColor()));
    blur.addSubview(&note);

    window.setContentView(Some(&blur));
    window.center();
    SettingsWindow {
        window,
        checks,
        preset_fields,
        content_popup,
        language_popup,
        subtitle,
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

/// Index of the popup entry for a title content.
pub fn content_index(content: TitleContent) -> usize {
    TitleContent::all()
        .iter()
        .position(|candidate| *candidate == content)
        .unwrap_or(0)
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
