use crate::buffer::Buffer;
use crate::error::{NodeError, NodeResult};
use crate::stream::{Readable, Writable};

#[derive(Debug, Clone, Default)]
pub struct SourceInterfaceOptions {
    pub input: Readable,
    pub output: Option<Writable>,
    pub terminal: Option<bool>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPos {
    pub rows: usize,
    pub cols: usize,
}

#[derive(Debug)]
pub struct Interface {
    input: Readable,
    output: Option<Writable>,
    pending_input: Vec<u8>,
    closed: bool,
    paused: bool,
    prompt: String,
    line: String,
    cursor: usize,
    terminal: bool,
}

impl Interface {
    pub fn create(options: SourceInterfaceOptions) -> Self {
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

    pub fn question_callable<E>(
        &mut self,
        query: &str,
        callback: tsonic_rust_runtime::Callable<(String,), Result<(), E>>,
    ) -> NodeResult<()>
    where
        E: std::fmt::Display + 'static,
    {
        self.write_output(query)?;
        if self.input.is_stdin_source() {
            return crate::background::spawn(
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
                move |answer| {
                    callback
                        .call((answer.map_err(tsonic_rust_runtime::TsonicError::from)?,))
                        .map_err(crate::error::callback_runtime_error)
                },
            );
        }
        let answer = self.next_line()?.ok_or_else(|| {
            NodeError::new(
                "ERR_READLINE_EOF",
                "readline input ended before an answer was available",
            )
        })?;
        crate::event_loop::enqueue_runtime_task(move || {
            callback
                .call((answer,))
                .map_err(crate::error::callback_runtime_error)
        })
    }

    pub fn write(&mut self, text: &str) -> NodeResult<()> {
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

    pub fn prompt(&mut self) -> NodeResult<()> {
        if !self.closed && !self.paused {
            let prompt = self.prompt.clone();
            self.write_output(&prompt)?;
        }
        Ok(())
    }

    pub fn line(&self) -> String {
        self.line.clone()
    }

    pub fn cursor_number(&self) -> f64 {
        self.cursor as f64
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

    pub fn next_line(&mut self) -> NodeResult<Option<String>> {
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
                return String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|error| NodeError::new("ERR_INVALID_UTF8", error.to_string()));
            }
            let Some(chunk) = self.input.read_result()? else {
                if self.pending_input.is_empty() {
                    return Ok(None);
                }
                let bytes = std::mem::take(&mut self.pending_input);
                return String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|error| NodeError::new("ERR_INVALID_UTF8", error.to_string()));
            };
            self.pending_input.extend_from_slice(&chunk.as_bytes());
        }
    }

    fn write_output(&mut self, text: &str) -> NodeResult<()> {
        let Some(output) = &mut self.output else {
            return Ok(());
        };
        let buffer = Buffer::from_string(text, Some("utf8"))?;
        if !output.write(buffer) {
            output.flush();
        }
        Ok(())
    }
}

pub fn create_interface(options: SourceInterfaceOptions) -> Interface {
    Interface::create(options)
}
