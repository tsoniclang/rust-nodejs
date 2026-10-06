impl<E: From<NodeError> + 'static> Readable<E> {
    pub fn wrap(readable: Readable<E>) -> Self {
        readable
    }

    pub fn iterator(&self) -> Result<Vec<Buffer>, E> {
        self.drain_remaining()
    }

    pub fn take(&self, limit: usize) -> Result<Vec<Buffer>, E> {
        let mut output = Vec::new();
        while output.len() < limit {
            let Some(chunk) = self.read()? else {
                break;
            };
            output.push(chunk);
        }
        Ok(output)
    }

    pub fn drop(&self, limit: usize) -> Result<Vec<Buffer>, E> {
        for _ in 0..limit {
            if self.read()?.is_none() {
                break;
            }
        }
        self.drain_remaining()
    }

    pub fn map(&self, mapper: impl Fn(Buffer) -> Buffer) -> Result<Readable<E>, E> {
        let options = self.state.borrow().options.clone();
        Ok(Readable::<E>::from_chunks_with_options(
            self.drain_remaining()?.into_iter().map(mapper).collect(),
            options,
        ))
    }

    pub fn filter(&self, predicate: impl Fn(&Buffer) -> bool) -> Result<Readable<E>, E> {
        let options = self.state.borrow().options.clone();
        Ok(Readable::<E>::from_chunks_with_options(
            self.drain_remaining()?
                .into_iter()
                .filter(predicate)
                .collect(),
            options,
        ))
    }

    pub fn flat_map(&self, mapper: impl Fn(Buffer) -> Vec<Buffer>) -> Result<Readable<E>, E> {
        let options = self.state.borrow().options.clone();
        Ok(Readable::<E>::from_chunks_with_options(
            self.drain_remaining()?
                .into_iter()
                .flat_map(mapper)
                .collect(),
            options,
        ))
    }

    pub fn for_each(&self, mut callback: impl FnMut(Buffer)) -> Result<(), E> {
        while let Some(chunk) = self.read()? {
            callback(chunk);
        }
        Ok(())
    }

    pub fn every(&self, predicate: impl Fn(&Buffer) -> bool) -> Result<bool, E> {
        Ok(self.drain_remaining()?.iter().all(predicate))
    }

    pub fn some(&self, predicate: impl Fn(&Buffer) -> bool) -> Result<bool, E> {
        Ok(self.drain_remaining()?.iter().any(predicate))
    }

    pub fn find(&self, predicate: impl Fn(&Buffer) -> bool) -> Result<Option<Buffer>, E> {
        Ok(self.drain_remaining()?.into_iter().find(predicate))
    }

    pub fn reduce<T>(&self, initial: T, reducer: impl Fn(T, Buffer) -> T) -> Result<T, E> {
        Ok(self.drain_remaining()?.into_iter().fold(initial, reducer))
    }

    pub fn compose(self, next: impl Fn(Readable<E>) -> Readable<E>) -> Readable<E> {
        next(self)
    }

    pub fn to_array(&self) -> Result<Vec<Buffer>, E> {
        self.drain_remaining()
    }

    pub fn to_vec(self) -> Result<Vec<Buffer>, E> {
        self.drain_remaining()
    }
}
