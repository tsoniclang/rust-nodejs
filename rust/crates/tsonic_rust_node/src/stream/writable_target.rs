pub trait WritableTarget: Clone {
    fn writable_handle(&self) -> Writable;
}

impl WritableTarget for Writable {
    fn writable_handle(&self) -> Writable {
        self.clone()
    }
}
