//! This mod provides the logic for the inner tree structure of the `CancellationToken`.
//!
//! `CancellationTokens` are only light handles with references to [`TreeNode`].
//! All the logic is actually implemented in the [`TreeNode`].
//!
//! A [`TreeNode`] is part of the cancellation tree and may have one parent and an arbitrary number of
//! children.
//!
//! A [`TreeNode`] can receive the request to perform a cancellation through a `CancellationToken`.
//! This cancellation request will cancel the node and all of its descendants.
//!
//! As soon as a node cannot get cancelled any more (because it was already cancelled or it has no
//! more `CancellationTokens` pointing to it any more), it gets removed from the tree, to keep the
//! tree as small as possible.
//!
//! # Invariants
//!
//! Those invariants shall be true at any time.
//!
//! 1. A node that has no parents and no handles can no longer be cancelled.
//!    This is important during both cancellation and refcounting.
//!
//! 2. If node B *is* or *was* a child of node A, then node B was created *after* node A.
//!    This is important for deadlock safety, as it is used for lock order.
//!    Node B can only become the child of node A in two ways:
//!    - being created with `child_node()`, in which case it is trivially true that
//!      node A already existed when node B was created
//!    - being moved A->C->B to A->B because node C was removed in `decrease_handle_refcount()`
//!      or `cancel()`. In this case the invariant still holds, as B was younger than C, and C
//!      was younger than A, therefore B is also younger than A.
//!
//! 3. If two nodes are both unlocked and node A is the parent of node B, then node B is a child of
//!    node A. It is important to always restore that invariant before dropping the lock of a node.
//!
//! # Deadlock safety
//!
//! We always lock in the order of creation time. We can prove this through invariant #2.
//! Specifically, through invariant #2, we know that we always have to lock a parent
//! before its child.
//!
use crate::loom::sync::{Arc, Mutex, MutexGuard};

/// A node of the cancellation tree structure
///
/// The actual data it holds is wrapped inside a mutex for synchronization.
pub(crate) struct TreeNode {
    inner: Mutex<Inner>,
    waker: tokio::sync::Notify,
}
impl TreeNode {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                parent: None,
                parent_idx: 0,
                children: vec![],
                is_cancelled: false,
                num_handles: 1,
            }),
            waker: tokio::sync::Notify::new(),
        }
    }

    pub(crate) fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.waker.notified()
    }
}

/// The data contained inside a `TreeNode`.
///
/// This struct exists so that the data of the node can be wrapped
/// in a Mutex.
struct Inner {
    parent: Option<Arc<TreeNode>>,
    parent_idx: usize,
    children: Vec<Arc<TreeNode>>,
    is_cancelled: bool,
    num_handles: usize,
}

/// Returns whether or not the node is cancelled
pub(crate) fn is_cancelled(node: &Arc<TreeNode>) -> bool {
    node.inner.lock().unwrap().is_cancelled
}

/// Creates a child node
pub(crate) fn child_node(parent: &Arc<TreeNode>) -> Arc<TreeNode> {
    let mut locked_parent = parent.inner.lock().unwrap();

    // Do not register as child if we are already cancelled.
    // Cancelled trees can never be uncancelled and therefore
    // need no connection to parents or children any more.
    if locked_parent.is_cancelled {
        return Arc::new(TreeNode {
            inner: Mutex::new(Inner {
                parent: None,
                parent_idx: 0,
                children: vec![],
                is_cancelled: true,
                num_handles: 1,
            }),
            waker: tokio::sync::Notify::new(),
        });
    }

    let child = Arc::new(TreeNode {
        inner: Mutex::new(Inner {
            parent: Some(parent.clone()),
            parent_idx: locked_parent.children.len(),
            children: vec![],
            is_cancelled: false,
            num_handles: 1,
        }),
        waker: tokio::sync::Notify::new(),
    });

    locked_parent.children.push(child.clone());

    child
}

/// Disconnects the given parent from all of its children.
///
/// Takes a reference to [Inner] to make sure the parent is already locked.
fn disconnect_children(node: &mut Inner) {
    for child in std::mem::take(&mut node.children) {
        let mut locked_child = child.inner.lock().unwrap();
        locked_child.parent_idx = 0;
        locked_child.parent = None;
    }
}

/// Figures out the parent of the node and locks the node and its parent atomically.
///
/// The basic principle of preventing deadlocks in the tree is
/// that we always lock the parent first, and then the child.
/// For more info look at *deadlock safety* and *invariant #2*.
///
/// Sadly, it's impossible to figure out the parent of a node without
/// locking it. To then achieve locking order consistency, the node
/// has to be unlocked before the parent gets locked.
/// This leaves a small window where we already assume that we know the parent,
/// but neither the parent nor the node is locked. Therefore, the parent could change.
///
/// To prevent that this problem leaks into the rest of the code, it is abstracted
/// in this function.
///
/// The locked child and optionally its locked parent, if a parent exists, get passed
/// to the `func` argument via (node, None) or (node, Some(parent)).
fn with_locked_node_and_parent<F, Ret>(node: &Arc<TreeNode>, func: F) -> Ret
where
    F: FnOnce(MutexGuard<'_, Inner>, Option<MutexGuard<'_, Inner>>) -> Ret,
{
    use std::sync::TryLockError;

    let mut locked_node = node.inner.lock().unwrap();

    // Every time this fails, the number of ancestors of the node decreases,
    // so the loop must succeed after a finite number of iterations.
    loop {
        // Look up the parent of the currently locked node.
        let potential_parent = match locked_node.parent.as_ref() {
            Some(potential_parent) => potential_parent.clone(),
            None => return func(locked_node, None),
        };

        // Lock the parent. This may require unlocking the child first.
        let locked_parent = match potential_parent.inner.try_lock() {
            Ok(locked_parent) => locked_parent,
            Err(TryLockError::WouldBlock) => {
                drop(locked_node);
                // Deadlock safety:
                //
                // Due to invariant #2, the potential parent must come before
                // the child in the creation order. Therefore, we can safely
                // lock the child while holding the parent lock.
                let locked_parent = potential_parent.inner.lock().unwrap();
                locked_node = node.inner.lock().unwrap();
                locked_parent
            }
            // https://github.com/tokio-rs/tokio/pull/6273#discussion_r1443752911
            #[allow(clippy::unnecessary_literal_unwrap)]
            Err(TryLockError::Poisoned(err)) => Err(err).unwrap(),
        };

        // If we unlocked the child, then the parent may have changed. Check
        // that we still have the right parent.
        if let Some(actual_parent) = locked_node.parent.as_ref() {
            if Arc::ptr_eq(actual_parent, &potential_parent) {
                return func(locked_node, Some(locked_parent));
            }
        }
    }
}

/// Moves all children from `node` to `parent`.
///
/// `parent` MUST have been a parent of the node when they both got locked,
/// otherwise there is a potential for a deadlock as invariant #2 would be violated.
///
/// To acquire the locks for node and parent, use [`with_locked_node_and_parent`].
fn move_children_to_parent(node: &mut Inner, parent: &mut Inner) {
    // Pre-allocate in the parent, for performance
    parent.children.reserve(node.children.len());

    for child in std::mem::take(&mut node.children) {
        {
            let mut child_locked = child.inner.lock().unwrap();
            child_locked.parent.clone_from(&node.parent);
            child_locked.parent_idx = parent.children.len();
        }
        parent.children.push(child);
    }
}

/// Removes a child from the parent.
///
/// `parent` MUST be the parent of `node`.
/// To acquire the locks for node and parent, use [`with_locked_node_and_parent`].
fn remove_child(parent: &mut Inner, mut node: MutexGuard<'_, Inner>) {
    // Query the position from where to remove a node
    let pos = node.parent_idx;
    node.parent = None;
    node.parent_idx = 0;

    // Unlock node, so that only one child at a time is locked.
    // Otherwise we would violate the lock order (see 'deadlock safety') as we
    // don't know the creation order of the child nodes
    drop(node);

    // If `node` is the last element in the list, we don't need any swapping
    if parent.children.len() == pos + 1 {
        parent.children.pop().unwrap();
    } else {
        // If `node` is not the last element in the list, we need to
        // replace it with the last element
        let replacement_child = parent.children.pop().unwrap();
        replacement_child.inner.lock().unwrap().parent_idx = pos;
        parent.children[pos] = replacement_child;
    }

    let len = parent.children.len();
    if 4 * len <= parent.children.capacity() {
        parent.children.shrink_to(2 * len);
    }
}

/// Increases the reference count of handles.
pub(crate) fn increase_handle_refcount(node: &Arc<TreeNode>) {
    let mut locked_node = node.inner.lock().unwrap();

    // Once no handles are left over, the node gets detached from the tree.
    // There should never be a new handle once all handles are dropped.
    assert!(locked_node.num_handles > 0);

    locked_node.num_handles += 1;
}

/// Decreases the reference count of handles.
///
/// Once no handle is left, we can remove the node from the
/// tree and connect its parent directly to its children.
pub(crate) fn decrease_handle_refcount(node: &Arc<TreeNode>) {
    let num_handles = {
        let mut locked_node = node.inner.lock().unwrap();
        locked_node.num_handles -= 1;
        locked_node.num_handles
    };

    if num_handles == 0 {
        with_locked_node_and_parent(node, |mut node, parent| {
            // Remove the node from the tree
            match parent {
                Some(mut parent) => {
                    // As we want to remove ourselves from the tree,
                    // we have to move the children to the parent, so that
                    // they still receive the cancellation event without us.
                    // Moving them does not violate invariant #1.
                    move_children_to_parent(&mut node, &mut parent);

                    // Remove the node from the parent
                    remove_child(&mut parent, node);
                }
                None => {
                    // Due to invariant #1, we can assume that our
                    // children can no longer be cancelled through us.
                    // (as we now have neither a parent nor handles)
                    // Therefore we can disconnect them.
                    disconnect_children(&mut node);
                }
            }
        });
    }
}

/// Cancels a node and its children.
pub(crate) fn cancel(node: &Arc<TreeNode>) {
    let mut locked_node = node.inner.lock().unwrap();

    if locked_node.is_cancelled {
        return;
    }

    // One by one, adopt grandchildren and then cancel and detach the child
    while let Some(child) = locked_node.children.pop() {
        // This can't deadlock because the mutex we are already
        // holding is the parent of child.
        let mut locked_child = child.inner.lock().unwrap();

        // Detach the child from node
        // No need to modify node.children, as the child already got removed with `.pop`
        locked_child.parent = None;
        locked_child.parent_idx = 0;

        // If child is already cancelled, detaching is enough
        if locked_child.is_cancelled {
            continue;
        }

        // Cancel or adopt grandchildren
        while let Some(grandchild) = locked_child.children.pop() {
            // This can't deadlock because the two mutexes we are already
            // holding is the parent and grandparent of grandchild.
            let mut locked_grandchild = grandchild.inner.lock().unwrap();

            // Detach the grandchild
            locked_grandchild.parent = None;
            locked_grandchild.parent_idx = 0;

            // If grandchild is already cancelled, detaching is enough
            if locked_grandchild.is_cancelled {
                continue;
            }

            // For performance reasons, only adopt grandchildren that have children.
            // Otherwise, just cancel them right away, no need for another iteration.
            if locked_grandchild.children.is_empty() {
                // Cancel the grandchild
                locked_grandchild.is_cancelled = true;
                locked_grandchild.children = Vec::new();
                drop(locked_grandchild);
                grandchild.waker.notify_waiters();
            } else {
                // Otherwise, adopt grandchild
                locked_grandchild.parent = Some(node.clone());
                locked_grandchild.parent_idx = locked_node.children.len();
                drop(locked_grandchild);
                locked_node.children.push(grandchild);
            }
        }

        // Cancel the child
        locked_child.is_cancelled = true;
        locked_child.children = Vec::new();
        drop(locked_child);
        child.waker.notify_waiters();

        // Now the child is cancelled and detached and all its children are adopted.
        // Just continue until all (including adopted) children are cancelled and detached.
    }

    // Cancel the node itself.
    locked_node.is_cancelled = true;
    locked_node.children = Vec::new();
    drop(locked_node);
    node.waker.notify_waiters();
}
