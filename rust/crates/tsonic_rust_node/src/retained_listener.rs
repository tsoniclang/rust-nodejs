use std::cell::RefCell;
use std::rc::Rc;

pub(crate) enum RetainedListener<Callback> {
    Repeated(Callback),
    Once(Rc<RefCell<Option<Callback>>>),
}

impl<Callback: Clone> Clone for RetainedListener<Callback> {
    fn clone(&self) -> Self {
        match self {
            Self::Repeated(callback) => Self::Repeated(callback.clone()),
            Self::Once(callback) => Self::Once(Rc::clone(callback)),
        }
    }
}

impl<Callback> RetainedListener<Callback> {
    pub(crate) fn new(callback: Callback, once: bool) -> Self {
        if once {
            Self::Once(Rc::new(RefCell::new(Some(callback))))
        } else {
            Self::Repeated(callback)
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        match self {
            Self::Repeated(_) => true,
            Self::Once(callback) => callback.borrow().is_some(),
        }
    }

    pub(crate) fn with_callback<Output>(
        &self,
        operation: impl FnOnce(&Callback) -> Output,
    ) -> Option<Output> {
        match self {
            Self::Repeated(callback) => Some(operation(callback)),
            Self::Once(callback) => {
                let selected = callback.borrow_mut().take();
                selected.as_ref().map(operation)
            }
        }
    }
}
