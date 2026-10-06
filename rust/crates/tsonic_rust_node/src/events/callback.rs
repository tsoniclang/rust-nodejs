use std::cell::RefCell;
use std::rc::Rc;
use tsonic_rust_js::JsValue;
use tsonic_rust_runtime::Callable;

pub(super) enum EventCallback<E: 'static> {
    Empty(Callable<(), Result<(), E>>),
    One(Callable<(JsValue,), Result<(), E>>),
    Two(Callable<(JsValue, JsValue), Result<(), E>>),
    Three(Callable<(JsValue, JsValue, JsValue), Result<(), E>>),
}

impl<E: 'static> Clone for EventCallback<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Empty(callback) => Self::Empty(callback.clone()),
            Self::One(callback) => Self::One(callback.clone()),
            Self::Two(callback) => Self::Two(callback.clone()),
            Self::Three(callback) => Self::Three(callback.clone()),
        }
    }
}

impl<E: 'static> EventCallback<E> {
    pub(super) fn invoke(&self, arguments: &[JsValue]) -> Result<(), E> {
        let value = |index| arguments.get(index).cloned().unwrap_or(JsValue::Null);
        match self {
            Self::Empty(callback) => callback.call(()),
            Self::One(callback) => callback.call((value(0),)),
            Self::Two(callback) => callback.call((value(0), value(1))),
            Self::Three(callback) => callback.call((value(0), value(1), value(2))),
        }
    }
}

pub(super) enum ListenerCallback<E: 'static> {
    Repeated(EventCallback<E>),
    Once(Rc<RefCell<Option<EventCallback<E>>>>),
}

impl<E: 'static> Clone for ListenerCallback<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Repeated(callback) => Self::Repeated(callback.clone()),
            Self::Once(callback) => Self::Once(Rc::clone(callback)),
        }
    }
}

impl<E: 'static> ListenerCallback<E> {
    pub(super) fn new(callback: EventCallback<E>, once: bool) -> Self {
        if once {
            Self::Once(Rc::new(RefCell::new(Some(callback))))
        } else {
            Self::Repeated(callback)
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        match self {
            Self::Repeated(_) => true,
            Self::Once(callback) => callback.borrow().is_some(),
        }
    }

    pub(super) fn invoke(&self, arguments: &[JsValue], before: &impl Fn()) -> Result<bool, E> {
        match self {
            Self::Repeated(callback) => {
                before();
                callback.invoke(arguments).map(|()| true)
            }
            Self::Once(callback) => {
                let selected = callback.borrow_mut().take();
                match selected {
                    Some(callback) => {
                        before();
                        callback.invoke(arguments).map(|()| true)
                    }
                    None => Ok(false),
                }
            }
        }
    }
}
