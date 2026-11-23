//! `GPUCommandBuffer`: what `encoder.finish()` makes and `queue.submit`
//! takes. No functions of its own.

use crate::registry::Id;

/// A command buffer in the registry: `None` once submitted, or when the
/// encoder could not finish (both make a later submit invalid).
pub struct CommandBufferEntry {
    /// The command buffer.
    pub buffer: Option<wgpu::CommandBuffer>,
    /// Its device.
    pub device: Id,
}
