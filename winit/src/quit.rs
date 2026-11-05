//! When to answer a macOS quit request.
//!
//! macOS waits for every quit request it routes to the application (Command+Q,
//! Dock ▸ Quit, logout, `terminate:`) until it gets one answer. [`Quit`] makes
//! sure each request gets exactly one, and that nothing is sent when none is
//! pending.

/// The answer to send to macOS: `true` quits, `false` keeps running.
pub(crate) type Answer = Option<bool>;

#[derive(Debug, Default)]
pub(crate) struct Quit {
    is_pending: bool,
}

impl Quit {
    /// A quit request arrived.
    ///
    /// It is answered at once (with "quit") when the event loop is already
    /// exiting or no `system::quit_requests` subscription would see it;
    /// otherwise it waits for [`Quit::exit`] or [`Quit::cancel`].
    pub(crate) fn request(&mut self, is_exiting: bool, has_listener: bool) -> Answer {
        if is_exiting || !has_listener {
            return Some(true);
        }

        self.is_pending = true;
        None
    }

    /// The application is exiting: a pending request is answered with "quit".
    pub(crate) fn exit(&mut self) -> Answer {
        self.take().then_some(true)
    }

    /// The application chose to stay: a pending request is answered with
    /// "keep running".
    pub(crate) fn cancel(&mut self) -> Answer {
        self.take().then_some(false)
    }

    fn take(&mut self) -> bool {
        std::mem::take(&mut self.is_pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_without_a_listener_quits_at_once() {
        let mut quit = Quit::default();

        assert_eq!(quit.request(false, false), Some(true));
        assert_eq!(quit.exit(), None, "nothing left pending");
    }

    #[test]
    fn a_request_while_exiting_quits_at_once() {
        let mut quit = Quit::default();

        assert_eq!(quit.request(true, true), Some(true));
        assert_eq!(quit.cancel(), None, "nothing left pending");
    }

    #[test]
    fn a_pending_request_is_answered_once() {
        let mut quit = Quit::default();
        assert_eq!(quit.request(false, true), None);

        assert_eq!(quit.cancel(), Some(false));
        assert_eq!(quit.cancel(), None, "no second answer");
        assert_eq!(quit.exit(), None, "no answer after cancel");
    }

    #[test]
    fn exit_answers_a_pending_request_with_quit() {
        let mut quit = Quit::default();
        assert_eq!(quit.request(false, true), None);

        assert_eq!(quit.exit(), Some(true));
        assert_eq!(quit.exit(), None, "no second answer");
    }

    #[test]
    fn cancel_and_exit_without_a_request_send_nothing() {
        let mut quit = Quit::default();

        assert_eq!(quit.cancel(), None);
        assert_eq!(quit.exit(), None);
    }
}
