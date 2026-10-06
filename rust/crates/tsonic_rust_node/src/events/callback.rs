use tsonic_rust_js::JsValue;
use tsonic_rust_runtime::Callable;

pub(crate) enum EventCallback<E: 'static> {
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
        self.invoke_with_values(|index| arguments.get(index).cloned().unwrap_or(JsValue::Null))
    }

    pub(crate) fn invoke_with_values(
        &self,
        mut value: impl FnMut(usize) -> JsValue,
    ) -> Result<(), E> {
        match self {
            Self::Empty(callback) => callback.call(()),
            Self::One(callback) => callback.call((value(0),)),
            Self::Two(callback) => callback.call((value(0), value(1))),
            Self::Three(callback) => callback.call((value(0), value(1), value(2))),
        }
    }
}

pub(crate) type ListenerCallback<E> = crate::retained_listener::RetainedListener<EventCallback<E>>;

impl<E: 'static> ListenerCallback<E> {
    pub(crate) fn invoke(&self, arguments: &[JsValue], before: &impl Fn()) -> Result<bool, E> {
        self.with_callback(|callback| {
            before();
            callback.invoke(arguments)
        })
        .transpose()
        .map(|result| result.is_some())
    }
}
