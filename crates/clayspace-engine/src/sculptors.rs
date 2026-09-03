//! The mesh sculptors a document is holding.
//!
//! A [`claycore::MeshSculptor`] is a weld and an adjacency pass over every
//! triangle a mesh layer carries, and on the reference form's 296,216 that is
//! 160 ms. The document used to hold exactly one, which was right while there
//! was one thing to sculpt: a second carried mesh evicted the first, so going
//! back and forth between two mesh subtools paid the pass on every switch —
//! against the 16 ms the specification allows an engine operation to hold the
//! interface thread, and now on a viewport click as well as a stack row.
//!
//! So the document holds several, and the switch a sculptor actually makes is
//! a lookup. The pass is still paid the first time a mesh subtool is worked
//! on, which is the cost of having a mesh at all; what has gone is paying it
//! again for a mesh already welded once.
//!
//! **The bound is a count, not bytes.** A sculptor's adjacency is proportional
//! to a mesh the document is already holding, so what it adds is a bounded
//! multiple of geometry that is resident either way; the count is the part
//! worth making predictable. Four covers going back and forth among a handful
//! of carried meshes, which is what a scene of subtools invites, and leaves
//! the coldest to be rebuilt on the rare visit back.
//!
//! Nothing here can hand back a sculptor built over a mesh that has since
//! changed. The engine guards that itself — its handle "remembers what it was
//! built over and every call checks that the answer has not changed", so a
//! layer whose triangles went out from under one is a refusal rather than a
//! read of freed storage — and [`Sculptors::forget`] is how this side keeps a
//! refusal from being reachable in the first place.
//!
//! **Shared rather than owned.** What is held is a handle behind an `Rc`, not
//! a sculptor, because a gesture in flight has to be able to keep the one it
//! is stamping with. A mesh gesture that defers its normals owes a flush to
//! the handle that deferred them, and taking the handle away — an eviction, a
//! layer removed under the drag — would leave the mesh shaded from where its
//! vertices used to be with nothing left to put it right. The gesture holds a
//! second reference, so the flush it owes cannot be separated from the thing
//! that owes it.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_model::LayerKey;

/// One mesh layer's sculptor, shared with whatever gesture is using it.
pub(crate) type SharedSculptor = Rc<RefCell<claycore::MeshSculptor>>;

/// The sculptors a document holds, least recently used first.
pub(crate) type Sculptors = Held<SharedSculptor>;

/// What is held is a type parameter so that the policy above — which one is
/// dropped, and when — can be exercised without an engine. A `MeshSculptor`
/// cannot be made without a mesh, and a bound that is only ever tested
/// through one is a bound nobody has actually read back.
pub(crate) struct Held<T> {
    held: Vec<(LayerKey, T)>,
}

// Derived would demand `T: Default`, which a sculptor is not.
impl<T> Default for Held<T> {
    fn default() -> Self {
        Self { held: Vec::new() }
    }
}

impl<T> Held<T> {
    /// How many are kept. See the note above on why this is a count.
    const KEPT: usize = 4;

    /// The sculptor for a layer, or `None` where none has been built.
    ///
    /// Asking counts as using it, which is what orders the eviction below:
    /// the layer a sculptor keeps coming back to is the last one dropped.
    pub(crate) fn get_mut(&mut self, layer: LayerKey) -> Option<&mut T> {
        let at = self.held.iter().position(|(key, _)| *key == layer)?;
        let entry = self.held.remove(at);
        self.held.push(entry);
        self.held.last_mut().map(|(_, sculptor)| sculptor)
    }

    /// Every sculptor held, in no order a caller should depend on.
    ///
    /// For reading a figure off all of them at once — the stale-seed count —
    /// rather than for reaching one, which is what `get_mut` is.
    pub(crate) fn values(&self) -> impl Iterator<Item = &T> {
        self.held.iter().map(|(_, held)| held)
    }

    /// How many are held.
    pub(crate) fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether one has been built for a layer, without counting as a use.
    pub(crate) fn holds(&self, layer: LayerKey) -> bool {
        self.held.iter().any(|(key, _)| *key == layer)
    }

    /// Takes one in, evicting the coldest to stay inside the bound.
    ///
    /// A layer that already had one keeps the new one: this is the path a
    /// rebuild after [`Sculptors::forget`] takes, and two entries under one
    /// key would leave the stale one reachable.
    pub(crate) fn insert(&mut self, layer: LayerKey, sculptor: T) {
        self.forget(layer);
        self.held.push((layer, sculptor));
        // A loop rather than one removal: the bound can be crossed by more
        // than one if it is ever lowered, and a `while` is correct at every
        // value of it.
        while self.held.len() > Self::KEPT {
            self.held.remove(0);
        }
    }

    /// Drops the sculptor for a layer, if there is one.
    ///
    /// Called where the mesh under it may have gone or been replaced — a
    /// removal, and a layer history brought back — because a sculptor built
    /// over geometry that is no longer there answers every call with a
    /// refusal, and a refusal reaching a sculptor as "the brush did nothing"
    /// is harder to place than a rebuild nobody notices.
    ///
    /// On the restore path this is a **guard rather than a fix**: measured,
    /// a layer this side removes and history brings back keeps the same
    /// `mesh::Mesh` behind its handle, so the sculptor held across it still
    /// resolves and `a_mesh_subtool_history_brings_back_can_be_sculpted`
    /// passes with this call taken out. Nothing in the ABI promises that —
    /// what it documents is the refusal for when it does not hold — so the
    /// rebuild is paid rather than the promise relied on.
    pub(crate) fn forget(&mut self, layer: LayerKey) {
        self.held.retain(|(key, _)| *key != layer);
    }

    /// Keeps only the layers a predicate names.
    ///
    /// The reconciliation after an undo is the caller: a layer that left the
    /// scene, or came back into it, is one whose mesh this side can no longer
    /// vouch for.
    pub(crate) fn retain(&mut self, keep: impl Fn(LayerKey) -> bool) {
        self.held.retain(|(key, _)| keep(*key));
    }

    /// The layers held, coldest first — the order eviction follows.
    #[cfg(test)]
    fn order(&self) -> Vec<LayerKey> {
        self.held.iter().map(|(key, _)| *key).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u64) -> LayerKey {
        LayerKey(n)
    }

    /// The whole point: a second mesh subtool does not cost the first one its
    /// sculptor, which is what made going back and forth pay the weld twice.
    #[test]
    fn a_second_layer_does_not_evict_the_first() {
        let mut held = Held::default();
        held.insert(key(1), "one");
        held.insert(key(2), "two");
        assert!(held.holds(key(1)));
        assert!(held.holds(key(2)));
    }

    #[test]
    fn the_coldest_is_the_one_dropped() {
        let mut held = Held::default();
        for n in 1..=Held::<&str>::KEPT as u64 + 1 {
            held.insert(key(n), "mesh");
        }
        assert_eq!(held.order().len(), Held::<&str>::KEPT);
        assert!(
            !held.holds(key(1)),
            "the first one in should be the first out"
        );
        assert!(held.holds(key(2)));
    }

    /// Asking for one is using it, so the layer a sculptor keeps returning to
    /// outlives layers it touched more recently but only once.
    #[test]
    fn asking_for_one_keeps_it_warm() {
        let mut held = Held::default();
        for n in 1..=Held::<&str>::KEPT as u64 {
            held.insert(key(n), "mesh");
        }
        assert!(held.get_mut(key(1)).is_some());
        held.insert(key(99), "mesh");
        assert!(held.holds(key(1)), "it was used most recently of all");
        assert!(!held.holds(key(2)), "which leaves this one the coldest");
    }

    /// A rebuild after `forget` must not leave the stale one reachable behind
    /// the new one.
    #[test]
    fn one_layer_is_held_once() {
        let mut held = Held::default();
        held.insert(key(1), "first");
        held.insert(key(1), "second");
        assert_eq!(held.order(), vec![key(1)]);
        assert_eq!(held.get_mut(key(1)).copied(), Some("second"));
    }

    #[test]
    fn forgetting_one_leaves_the_rest() {
        let mut held = Held::default();
        held.insert(key(1), "one");
        held.insert(key(2), "two");
        held.forget(key(1));
        assert!(!held.holds(key(1)));
        assert!(held.holds(key(2)));
    }

    #[test]
    fn retain_keeps_only_what_is_named() {
        let mut held = Held::default();
        held.insert(key(1), "one");
        held.insert(key(2), "two");
        held.insert(key(3), "three");
        held.retain(|layer| layer != key(2));
        assert_eq!(held.order(), vec![key(1), key(3)]);
    }
}
