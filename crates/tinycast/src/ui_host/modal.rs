//! One confirmation at a time. The dialog continues from this slot instead of
//! nesting another message pump.

pub struct ModalGate {
    pending: Option<Pending>,
}

pub struct Pending {
    pub on_accept: Box<dyn FnOnce()>,
    pub on_cancel: Box<dyn FnOnce()>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AlreadyOpen;

impl ModalGate {
    pub fn new() -> Self {
        Self { pending: None }
    }

    pub fn open(&mut self, pending: Pending) -> Result<(), AlreadyOpen> {
        if self.pending.is_some() {
            return Err(AlreadyOpen);
        }
        self.pending = Some(pending);
        Ok(())
    }

    pub fn accept(&mut self) {
        if let Some(pending) = self.pending.take() {
            (pending.on_accept)();
        }
    }

    pub fn cancel(&mut self) {
        if let Some(pending) = self.pending.take() {
            (pending.on_cancel)();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::{AlreadyOpen, ModalGate, Pending};

    fn counters() -> (Rc<Cell<u32>>, Rc<Cell<u32>>, Pending) {
        let accept = Rc::new(Cell::new(0));
        let cancel = Rc::new(Cell::new(0));
        let accept_hit = Rc::clone(&accept);
        let cancel_hit = Rc::clone(&cancel);
        let pending = Pending {
            on_accept: Box::new(move || accept_hit.set(accept_hit.get() + 1)),
            on_cancel: Box::new(move || cancel_hit.set(cancel_hit.get() + 1)),
        };
        (accept, cancel, pending)
    }

    #[test]
    fn accept_calls_only_on_accept() {
        let (accept, cancel, pending) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        gate.accept();
        assert_eq!(accept.get(), 1);
        assert_eq!(cancel.get(), 0);
    }

    #[test]
    fn cancel_calls_only_on_cancel() {
        let (accept, cancel, pending) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        gate.cancel();
        assert_eq!(accept.get(), 0);
        assert_eq!(cancel.get(), 1);
    }

    #[test]
    fn second_open_returns_already_open_without_calling() {
        let (accept, cancel, pending) = counters();
        let (second_accept, second_cancel, second) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        assert_eq!(gate.open(second), Err(AlreadyOpen));
        assert_eq!(accept.get(), 0);
        assert_eq!(cancel.get(), 0);
        assert_eq!(second_accept.get(), 0);
        assert_eq!(second_cancel.get(), 0);
    }

    #[test]
    fn accept_allows_another_open() {
        let (accept, cancel, pending) = counters();
        let (again_accept, again_cancel, again) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        gate.accept();
        gate.open(again).unwrap();
        gate.cancel();
        assert_eq!(accept.get(), 1);
        assert_eq!(cancel.get(), 0);
        assert_eq!(again_accept.get(), 0);
        assert_eq!(again_cancel.get(), 1);
    }
}
