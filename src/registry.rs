//! The handle registry: every WebGPU object TSX holds is a number, an id
//! into one table of the process, which keeps the wgpu object it stands
//! for.
//!
//! Ids are unique across kinds, so a descriptor can name any object by its
//! id alone (a bind group entry's `resource` is a sampler, a view or a
//! buffer), and the table tells the kind by downcasting. A kind's entry
//! type lives in its own file (`api/buffer.rs` keeps `BufferEntry`); this
//! file knows none of them, so areas add kinds without touching it.
//!
//! Locking: the table's lock is held only to insert, clone out or borrow
//! an entry. Calls into wgpu that can report an error or wait are made
//! outside it where the entry can be cloned (wgpu handles are reference
//! counted); [`with_mut`] holds it for the entries that cannot (encoders
//! and passes), whose calls neither wait nor reach the table again.

use std::{
    any::Any,
    collections::BTreeMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU32, Ordering},
    },
};

use crate::error::NativeError;

/// An object id as TSX holds it.
pub type Id = u32;

/// The table: id -> entry of any kind.
static OBJECTS: Mutex<BTreeMap<Id, Box<dyn Any + Send>>> = Mutex::new(BTreeMap::new());

/// The next id; 0 is never handed out (TSX reads it as "none").
static NEXT: AtomicU32 = AtomicU32::new(1);

fn objects() -> MutexGuard<'static, BTreeMap<Id, Box<dyn Any + Send>>> {
    // A panic while the lock was held (caught at the boundary) leaves the
    // table as it was: the entries are plain values.
    OBJECTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// A fresh id, for an object or a ticket.
pub(crate) fn next_id() -> Id {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Keeps `entry`; its id.
pub(crate) fn insert<T: Any + Send>(entry: T) -> Id {
    let id = next_id();
    objects().insert(id, Box::new(entry));
    id
}

/// Keeps `entry` under `id`, an id [`next_id`] gave (for an entry that
/// must know its own id when it is made: a device's error sink).
pub(crate) fn insert_at<T: Any + Send>(id: Id, entry: T) {
    objects().insert(id, Box::new(entry));
}

/// The name a kind goes by in errors: the last path segment of its type.
fn kind<T>() -> &'static str {
    let name = std::any::type_name::<T>();
    name.rsplit("::").next().unwrap_or(name)
}

/// A clone of entry `id`, which must be a `T`.
pub(crate) fn get<T: Any + Send + Clone>(id: Id) -> Result<T, NativeError> {
    let objects = objects();
    let entry = objects
        .get(&id)
        .ok_or_else(|| NativeError::type_error(format!("no object {id} (released?)")))?;
    entry
        .downcast_ref::<T>()
        .cloned()
        .ok_or_else(|| NativeError::type_error(format!("object {id} is not a {}", kind::<T>())))
}

/// Whether entry `id` exists, of any kind.
pub(crate) fn contains(id: Id) -> bool {
    objects().contains_key(&id)
}

/// Runs `f` on entry `id`, which must be a `T`, with the table locked.
pub(crate) fn with_mut<T: Any + Send, R>(
    id: Id,
    f: impl FnOnce(&mut T) -> R,
) -> Result<R, NativeError> {
    let mut objects = objects();
    let entry = objects
        .get_mut(&id)
        .ok_or_else(|| NativeError::type_error(format!("no object {id} (released?)")))?;
    let entry = entry
        .downcast_mut::<T>()
        .ok_or_else(|| NativeError::type_error(format!("object {id} is not a {}", kind::<T>())))?;
    Ok(f(entry))
}

/// Drops entry `id`, of any kind (TSX let its object go). The wgpu object
/// lives on while other entries or recorded work hold it.
pub(crate) fn release(id: Id) {
    let entry = objects().remove(&id);
    // Dropped outside the lock: a wgpu drop may report an error.
    drop(entry);
}

/// Takes entry `id` out of the table when it is a `T`; `None` otherwise.
pub(crate) fn take<T: Any + Send>(id: Id) -> Option<T> {
    let mut objects = objects();
    if !objects.get(&id).is_some_and(|e| e.is::<T>()) {
        return None;
    }
    objects
        .remove(&id)
        .and_then(|e| e.downcast::<T>().ok())
        .map(|e| *e)
}

/// How many entries of kind `T` the table holds.
pub(crate) fn count<T: Any + Send>() -> usize {
    objects().values().filter(|e| e.is::<T>()).count()
}

/// How many entries the table holds (tests check releases with it).
pub(crate) fn len() -> usize {
    objects().len()
}
