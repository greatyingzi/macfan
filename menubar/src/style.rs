//! Shared look and feel: fonts, colours, attributed menu titles, SF Symbols.
//!
//! AppKit menus take plain strings, so anything richer has to be built as an
//! attributed string. Centralising that keeps the two places that render text
//! (the menu and the settings window) consistent, and lets the number handling
//! live in one place: sensor readings are rendered with a monospaced-digit font
//! so they stop jittering as the value changes every two seconds.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AllocAnyThread;
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSFontWeightRegular, NSForegroundColorAttributeName,
    NSImage,
};
use objc2_foundation::{NSMutableAttributedString, NSRange, NSString};

/// How a piece of text should look.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// Normal informational text.
    Body,
    /// Emphasised (the "current state" line).
    Strong,
    /// Numeric run: even digits, so values do not shift around.
    Digit,
    /// De-emphasised (labels, units).
    Muted,
    /// Accent colour, for the active mode.
    Accent,
    /// Warmer colour, for heat.
    Warm,
}

/// Font for informational text.
fn body_font() -> Retained<NSFont> {
    NSFont::systemFontOfSize(12.0)
}

/// Font for emphasised text.
fn strong_font() -> Retained<NSFont> {
    NSFont::boldSystemFontOfSize(12.0)
}

/// Font with monospaced digits, for readings that change.
fn digit_font() -> Retained<NSFont> {
    NSFont::monospacedDigitSystemFontOfSize_weight(12.0, unsafe { NSFontWeightRegular })
}

impl Slot {
    fn attributes(self) -> (Retained<NSFont>, Retained<NSColor>) {
        match self {
            Slot::Body => (body_font(), NSColor::labelColor()),
            Slot::Strong => (strong_font(), NSColor::labelColor()),
            Slot::Digit => (digit_font(), NSColor::labelColor()),
            Slot::Muted => (body_font(), NSColor::secondaryLabelColor()),
            Slot::Accent => (strong_font(), NSColor::controlAccentColor()),
            Slot::Warm => (digit_font(), NSColor::systemOrangeColor()),
        }
    }
}

/// Split a line into the runs that should use the digit font and the rest.
///
/// Sensor values are the only thing that moves between refreshes, so giving
/// them even digits keeps a menu from reflowing under the pointer.
pub fn split_digits(line: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = false;
    for c in line.chars() {
        let is_digit = c.is_ascii_digit() || (c == '.' || c == '-' || c == '°' || c == 'C');
        // `°C` follows a number and belongs with it; a leading minus too.
        if out.is_empty() && current.is_empty() && (c == '-' || c == '°') {
            current_is_digit = true;
        }
        if is_digit != current_is_digit && !current.is_empty() {
            out.push((std::mem::take(&mut current), current_is_digit));
        }
        current_is_digit = is_digit;
        current.push(c);
    }
    if !current.is_empty() {
        out.push((current, current_is_digit));
    }
    out
}

/// Build an attributed string from styled pieces.
pub fn attributed(pieces: &[(String, Slot)]) -> Retained<NSMutableAttributedString> {
    let joined: String = pieces.iter().map(|(text, _)| text.as_str()).collect();
    let string = NSString::from_str(&joined);
    let result =
        NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &string);
    let mut location = 0usize;
    for (text, slot) in pieces {
        let length = text.encode_utf16().count();
        if length == 0 {
            continue;
        }
        let (font, color) = slot.attributes();
        let range = NSRange::new(location, length);
        unsafe {
            result.addAttribute_value_range(
                NSFontAttributeName,
                &*(Retained::as_ptr(&font) as *const AnyObject),
                range,
            );
            result.addAttribute_value_range(
                NSForegroundColorAttributeName,
                &*(Retained::as_ptr(&color) as *const AnyObject),
                range,
            );
        }
        location += length;
    }
    result
}

/// A line where digit runs get the even-digit font.
pub fn attributed_line(line: &str, slot: Slot) -> Retained<NSMutableAttributedString> {
    let pieces: Vec<(String, Slot)> = split_digits(line)
        .into_iter()
        .map(|(text, is_digit)| {
            let slot = if is_digit { Slot::Digit } else { slot };
            (text, slot)
        })
        .collect();
    attributed(&pieces)
}

/// An SF Symbol as a template image (so it follows the menu's appearance), or
/// `None` when the running system has none of the given names.
pub fn symbol(names: &[&str]) -> Option<Retained<NSImage>> {
    for name in names {
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(name),
            None,
        );
        if let Some(image) = image {
            image.setTemplate(true);
            return Some(image);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rebuilt(line: &str) -> String {
        split_digits(line).into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn splitting_never_loses_text() {
        for line in [
            "风扇 0：7195 rpm（目标 7199，强制）",
            "Fan 0: 7169 rpm (target 7199, forced)",
            "66°C",
            "no digits here",
            "",
            "-206 rpm",
        ] {
            assert_eq!(rebuilt(line), line);
        }
    }

    #[test]
    fn digits_are_isolated_but_cjk_labels_stay_whole() {
        let parts = split_digits("风扇 0：7195 rpm");
        assert_eq!(parts[0], ("风扇 ".to_string(), false));
        assert_eq!(parts[1], ("0".to_string(), true));
        assert_eq!(parts[2], ("：".to_string(), false));
        assert_eq!(parts[3], ("7195".to_string(), true));
        assert_eq!(parts[4], (" rpm".to_string(), false));
    }

    #[test]
    fn a_temperature_keeps_its_unit_with_the_number() {
        let parts = split_digits("66°C");
        assert_eq!(parts, vec![("66°C".to_string(), true)]);
    }
}
