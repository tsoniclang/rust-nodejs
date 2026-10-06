use crate::buffer::Buffer;
use crate::error::NodeError;
use crate::stream::{Readable, Writable};

pub struct SourceInterfaceOptions<E: 'static = NodeError> {
    pub input: Readable<E>,
    pub output: Option<Writable<E>>,
    pub terminal: Option<bool>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPos {
    pub rows: usize,
    pub cols: usize,
}

pub struct Interface<E: 'static = NodeError> {
    input: Readable<E>,
    output: Option<Writable<E>>,
    pending_input: Vec<u8>,
    closed: bool,
    paused: bool,
    prompt: String,
    line: String,
    cursor: usize,
    terminal: bool,
}

impl<E: From<NodeError> + 'static> Interface<E> {
    pub fn create(options: SourceInterfaceOptions<E>) -> Self {
        Self {
            input: options.input,
            output: options.output,
            pending_input: Vec::new(),
            closed: false,
            paused: false,
            prompt: options.prompt.unwrap_or_else(|| "> ".to_string()),
            line: String::new(),
            cursor: 0,
            terminal: options.terminal.unwrap_or(false),
        }
    }

    pub fn question_callable(
        &mut self,
        background: &crate::background::BackgroundTasks<E>,
        tasks: &crate::runtime_tasks::RuntimeTasks<E>,
        query: &str,
        callback: tsonic_rust_runtime::Callable<(String,), Result<(), E>>,
    ) -> Result<(), E> {
        self.write_output(query)?;
        if self.input.is_stdin_source() {
            return background
                .spawn(
                    || {
                        let mut answer = String::new();
                        std::io::stdin()
                            .read_line(&mut answer)
                            .map_err(|error| NodeError::new("EIO", error.to_string()))?;
                        if answer.ends_with('\n') {
                            answer.pop();
                            if answer.ends_with('\r') {
                                answer.pop();
                            }
                        }
                        Ok(answer)
                    },
                    move |answer| callback.call((answer.map_err(E::from)?,)),
                )
                .map_err(E::from);
        }
        let answer = self.next_line()?.ok_or_else(|| {
            NodeError::new(
                "ERR_READLINE_EOF",
                "readline input ended before an answer was available",
            )
        })?;
        tasks
            .enqueue(move || callback.call((answer,)))
            .map_err(E::from)
    }

    pub fn write(&mut self, text: &str) -> Result<(), E> {
        if self.closed || self.paused {
            return Ok(());
        }
        self.write_output(text)?;
        self.line.push_str(text);
        self.cursor = self.line.encode_utf16().count();
        Ok(())
    }

    pub fn pause_chain(&mut self) -> &mut Self {
        self.paused = true;
        self
    }

    pub fn resume_chain(&mut self) -> &mut Self {
        self.paused = false;
        self
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
    }

    pub fn get_prompt(&self) -> String {
        self.prompt.clone()
    }

    pub fn prompt(&mut self) -> Result<(), E> {
        if !self.closed && !self.paused {
            let prompt = self.prompt.clone();
            self.write_output(&prompt)?;
        }
        Ok(())
    }

    pub fn line(&self) -> String {
        self.line.clone()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn terminal(&self) -> bool {
        self.terminal
    }

    pub fn get_cursor_pos(&self) -> CursorPos {
        CursorPos {
            rows: 0,
            cols: self.cursor,
        }
    }

    pub fn next_line(&mut self) -> Result<Option<String>, E> {
        if self.closed || self.paused {
            return Ok(None);
        }
        loop {
            if let Some(newline) = self.pending_input.iter().position(|byte| *byte == b'\n') {
                let mut bytes = self.pending_input.drain(..=newline).collect::<Vec<_>>();
                bytes.pop();
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
                return String::from_utf8(bytes).map(Some).map_err(|error| {
                    E::from(NodeError::new("ERR_INVALID_UTF8", error.to_string()))
                });
            }
            let Some(chunk) = self.input.read()? else {
                if self.pending_input.is_empty() {
                    return Ok(None);
                }
                let bytes = std::mem::take(&mut self.pending_input);
                return String::from_utf8(bytes).map(Some).map_err(|error| {
                    E::from(NodeError::new("ERR_INVALID_UTF8", error.to_string()))
                });
            };
            self.pending_input.extend_from_slice(&chunk.as_bytes());
        }
    }

    fn write_output(&mut self, text: &str) -> Result<(), E> {
        let Some(output) = &mut self.output else {
            return Ok(());
        };
        let buffer = Buffer::from_string(text, Some("utf8"))?;
        if !output.write(buffer)? {
            output.flush()?;
        }
        Ok(())
    }
}

pub fn create_interface<E: From<NodeError> + 'static>(
    options: SourceInterfaceOptions<E>,
) -> Interface<E> {
    Interface::<E>::create(options)
}

impl<E: 'static> Clone for SourceInterfaceOptions<E> {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            output: self.output.clone(),
            terminal: self.terminal,
            prompt: self.prompt.clone(),
        }
    }
}
impl<E: From<NodeError> + 'static> Default for SourceInterfaceOptions<E> {
    fn default() -> Self {
        Self {
            input: Readable::default(),
            output: None,
            terminal: None,
            prompt: None,
        }
    }
}
impl<E: 'static> std::fmt::Debug for SourceInterfaceOptions<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceInterfaceOptions")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("terminal", &self.terminal)
            .field("prompt", &self.prompt)
            .finish()
    }
}
impl<E: 'static> std::fmt::Debug for Interface<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Interface")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("closed", &self.closed)
            .field("paused", &self.paused)
            .field("line", &self.line)
            .finish()
    }
}
