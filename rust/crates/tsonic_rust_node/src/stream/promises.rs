use super::{
    finished_with_options as finished_sync_with_options, pipeline as pipeline_sync,
    FinishedOptions, Readable, Writable,
};
use crate::buffer::Buffer;
use crate::error::NodeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineOptions {
    pub end: bool,
    pub signal_aborted: bool,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            end: true,
            signal_aborted: false,
        }
    }
}

pub fn pipeline<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    writable: &mut Writable<E>,
) -> Result<(), E> {
    pipeline_sync(readable, writable)
}

pub fn pipeline_with_options<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    writable: &mut Writable<E>,
    options: &PipelineOptions,
) -> Result<usize, E> {
    pipeline_transforms(readable, &[], writable, options)
}

pub fn pipeline_transform<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    transform: impl FnMut(Buffer) -> Buffer,
    writable: &mut Writable<E>,
    options: &PipelineOptions,
) -> Result<usize, E> {
    pipeline_transform_impl(readable, transform, writable, options)
}

pub fn pipeline_transforms<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    transforms: &[fn(Buffer) -> Buffer],
    writable: &mut Writable<E>,
    options: &PipelineOptions,
) -> Result<usize, E> {
    pipeline_transform_impl(
        readable,
        |mut chunk| {
            for transform in transforms {
                chunk = transform(chunk);
            }
            chunk
        },
        writable,
        options,
    )
}

pub fn finished<E: From<NodeError> + 'static>(
    readable: &Readable<E>,
    writable: &Writable<E>,
) -> bool {
    super::finished(readable, writable)
}

pub fn finished_with_options<E: From<NodeError> + 'static>(
    readable: &Readable<E>,
    writable: &Writable<E>,
    options: &FinishedOptions,
) -> bool {
    finished_sync_with_options(readable, writable, options)
}

fn pipeline_transform_impl<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    mut transform: impl FnMut(Buffer) -> Buffer,
    writable: &mut Writable<E>,
    options: &PipelineOptions,
) -> Result<usize, E> {
    if options.signal_aborted {
        return Err(NodeError::new("ABORT_ERR", "pipeline aborted").into());
    }

    let mut written = 0;
    while let Some(chunk) = readable.read()? {
        if !writable.write(transform(chunk))? {
            written += 1;
            break;
        }
        written += 1;
    }
    if options.end {
        writable.end()?;
    }
    Ok(written)
}
