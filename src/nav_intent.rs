//! Pending in-app navigation captured from Android `navigate()` or a location href write.
//! The Kotlin shell reads this after a click and re-runs the hook with the new path.

use std::cell::RefCell;

thread_local! {
    static PENDING_HREF: RefCell<Option<String>> = RefCell::new(None);
}

pub fn note_location_href(path: &str) {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return;
    }
    PENDING_HREF.with(|c| *c.borrow_mut() = Some(trimmed.to_string()));
}

pub fn take_location_href() -> Option<String> {
    PENDING_HREF.with(|c| c.borrow_mut().take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn href_is_taken_once() {
        let _ = take_location_href();
        note_location_href("  /alpha  ");
        assert_eq!(take_location_href().as_deref(), Some("/alpha"));
        assert!(take_location_href().is_none());
        note_location_href("   ");
        assert!(take_location_href().is_none());
    }
}
