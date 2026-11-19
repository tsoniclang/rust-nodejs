use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::buffer::Buffer;
use crate::stream::{Readable, WeakReadable};
use crate::{NodeError, NodeResult};
use tsonic_rust_runtime::Callable;

type Answer<E> = Callable<'static, (String,), Result<(), E>>;

pub(super) struct Question<E: From<NodeError> + 'static> {
    input: WeakReadable<E>,
    bytes: RefCell<Vec<u8>>,
    callback: RefCell<Option<Answer<E>>>,
    data: Cell<usize>,
    end: Cell<usize>,
    done: Cell<bool>,
}

impl<E: From<NodeError> + 'static> Question<E> {
    pub(super) fn start(
        input: Readable<E>,
        bytes: Vec<u8>,
        callback: Answer<E>,
    ) -> Result<Rc<Self>, E> {
        let question = Rc::new(Self {
            input: input.downgrade(),
            bytes: RefCell::new(bytes),
            callback: RefCell::new(Some(callback)),
            data: Cell::new(0),
            end: Cell::new(0),
            done: Cell::new(false),
        });
        let observer = Rc::clone(&question);
        let data = Callable::new(move |(chunk,): (Buffer,)| {
            if observer.done.get() {
                return Ok(());
            }
            chunk.with_bytes(|bytes| observer.bytes.borrow_mut().extend_from_slice(bytes));
            observer.complete_line(false)
        });
        let observer = Rc::clone(&question);
        let end = Callable::new(move |()| observer.complete_line(true));
        question.data.set(data.identity_key());
        question.end.set(end.identity_key());
        let registered = (|| {
            input.on_data("data", &data)?;
            input.once_end("end", &end)?;
            input.resume()?;
            Ok(())
        })();
        if let Err(error) = registered {
            question.cancel();
            return Err(error);
        }
        Ok(question)
    }

    pub(super) fn pending(&self) -> bool {
        !self.done.get() && self.input.upgrade().is_some_and(|input| !input.destroyed())
    }

    fn complete_line(&self, complete: bool) -> Result<(), E> {
        if self.done.get() {
            return Ok(());
        }
        let line = {
            let mut bytes = self.bytes.borrow_mut();
            match bytes.iter().position(|byte| *byte == b'\n') {
                Some(newline) => {
                    let remaining = bytes.split_off(newline + 1);
                    let mut line = std::mem::replace(&mut *bytes, remaining);
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    Some(line)
                }
                None if complete => Some(std::mem::take(&mut *bytes)),
                None => None,
            }
        };
        let Some(line) = line else {
            return Ok(());
        };
        let answer: NodeResult<String> = if complete && line.is_empty() {
            Err(NodeError::new(
                "ERR_READLINE_EOF",
                "readline input ended before an answer was available",
            ))
        } else {
            String::from_utf8(line)
                .map_err(|error| NodeError::new("ERR_INVALID_UTF8", error.to_string()))
        };
        self.cancel();
        let remaining = std::mem::take(&mut *self.bytes.borrow_mut());
        if !remaining.is_empty() {
            let input = self.input.upgrade().ok_or_else(|| {
                NodeError::new(
                    "ERR_STREAM_DESTROYED",
                    "readline input was released before its unread data",
                )
            })?;
            input.unshift(Buffer::from_bytes(remaining))?;
        }
        let callback = self
            .callback
            .borrow_mut()
            .take()
            .expect("one native answer callback");
        callback.call((answer.map_err(E::from)?,))
    }

    pub(super) fn cancel(&self) {
        if self.done.replace(true) {
            return;
        }
        if let Some(input) = self.input.upgrade() {
            input.pause();
            input.remove_readable_listeners(self.data.get(), self.end.get());
        }
    }
}

impl<E: From<NodeError> + 'static> Drop for Question<E> {
    fn drop(&mut self) {
        self.cancel();
    }
}
