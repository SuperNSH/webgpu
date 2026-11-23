//! Work TSX awaits: `mapAsync`, `onSubmittedWorkDone`. The call that
//! starts it runs in the JS call (so validation errors reach the device's
//! scopes in order) and returns a ticket; TSX then awaits `Wgpu.wait`
//! (`api/gpu.rs`), which polls the device off the JS thread until wgpu's
//! callback has filled the ticket's slot.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::{
    error::NativeError,
    registry::{Id, next_id},
};

/// Where wgpu's callback puts the outcome (the first one put wins).
pub(crate) type Slot = Arc<Mutex<Option<Result<(), NativeError>>>>;

struct Ticket {
    device: wgpu::Device,
    slot: Slot,
}

static TICKETS: Mutex<BTreeMap<Id, Ticket>> = Mutex::new(BTreeMap::new());

fn tickets() -> std::sync::MutexGuard<'static, BTreeMap<Id, Ticket>> {
    TICKETS.lock().unwrap_or_else(|e| e.into_inner())
}

/// A ticket for work on `device`, and the slot its callback fills.
pub(crate) fn start(device: &wgpu::Device) -> (Id, Slot) {
    let slot: Slot = Arc::default();
    let id = next_id();
    tickets().insert(
        id,
        Ticket {
            device: device.clone(),
            slot: Arc::clone(&slot),
        },
    );
    (id, slot)
}

/// A ticket already settled with `outcome` (work rejected before it
/// started).
pub(crate) fn settled(device: &wgpu::Device, outcome: Result<(), NativeError>) -> Id {
    let (id, slot) = start(device);
    fill(&slot, outcome);
    id
}

/// Puts `outcome` in `slot` unless one is there.
pub(crate) fn fill(slot: &Slot, outcome: Result<(), NativeError>) {
    slot.lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert(outcome);
}

/// The outcome of ticket `id` if it is settled (the ticket is gone then),
/// else the device to poll.
pub(crate) fn check(id: Id) -> Result<Result<Result<(), NativeError>, wgpu::Device>, NativeError> {
    let mut tickets = tickets();
    let ticket = tickets
        .get(&id)
        .ok_or_else(|| NativeError::type_error(format!("no pending work {id}")))?;
    let outcome = ticket.slot.lock().unwrap_or_else(|e| e.into_inner()).take();
    Ok(match outcome {
        Some(outcome) => {
            tickets.remove(&id);
            Ok(outcome)
        }
        None => Err(ticket.device.clone()),
    })
}

/// Drops ticket `id`, settled or not (its waiter gave up).
pub(crate) fn forget(id: Id) {
    let ticket = tickets().remove(&id);
    // Dropped outside the lock: it holds a device.
    drop(ticket);
}

/// How many tickets are kept (tests check that none is left behind).
pub fn count() -> usize {
    tickets().len()
}
