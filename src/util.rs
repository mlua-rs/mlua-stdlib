pub(crate) struct Defer<F: FnOnce() -> R, R>(Option<F>);

impl<F: FnOnce() -> R, R> Drop for Defer<F, R> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

pub(crate) fn defer<F: FnOnce() -> R, R>(f: F) -> Defer<F, R> {
    Defer(Some(f))
}
