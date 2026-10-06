pub trait WritableTarget<E: 'static = NodeError>: Clone {
    fn writable_handle(&self) -> Writable<E>;
}

impl<E: 'static> WritableTarget<E> for Writable<E> {
    fn writable_handle(&self) -> Writable<E> {
        self.clone()
    }
}
