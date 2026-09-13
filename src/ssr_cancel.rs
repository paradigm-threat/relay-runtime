//! Cooperative SSR cancel — wall-clock timeout sets a flag; hot loops abort instead of orphaning CPU.
//!
//! The gateway installs an [`Arc<AtomicBool>`] on the `relay-ssr` worker via [`install_ssr_cancel`].
//! On `recv_timeout` it stores `true` on that Arc; markdown / stable-pass checks [`ssr_is_cancelled`]
//! (thread-local install) and bail so the permit drops promptly.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

thread_local! {
    /// Cancel flag for the SSR worker running on this OS thread.
    static CURRENT: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

/// Install (or clear with `None`) the cancel flag for this thread's SSR work.
pub fn install_ssr_cancel(flag: Option<Arc<AtomicBool>>) {
    CURRENT.with(|c| {
        *c.borrow_mut() = flag;
    });
}

/// True when this thread has an SSR cancel Arc installed (gateway `relay-ssr` worker).
pub fn ssr_cancel_installed() -> bool {
    CURRENT.with(|c| c.borrow().is_some())
}

/// True when this thread's installed cancel flag is set.
pub fn ssr_is_cancelled() -> bool {
    CURRENT.with(|c| {
        c.borrow()
            .as_ref()
            .map(|f| f.load(Ordering::Acquire))
            .unwrap_or(false)
    })
}

/// Signal cancel on the given flag (used by the waiting HTTP thread after wall timeout).
pub fn signal_ssr_cancel(flag: &Arc<AtomicBool>) {
    flag.store(true, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_signal() {
        let f = Arc::new(AtomicBool::new(false));
        install_ssr_cancel(Some(f.clone()));
        assert!(ssr_cancel_installed());
        assert!(!ssr_is_cancelled());
        signal_ssr_cancel(&f);
        assert!(ssr_is_cancelled());
        install_ssr_cancel(None);
        assert!(!ssr_cancel_installed());
        assert!(!ssr_is_cancelled());
    }
}
