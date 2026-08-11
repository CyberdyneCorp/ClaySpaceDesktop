//! Authoring a ZSphere rig, as an interaction.
//!
//! The feel this is written for is ZBrush's: you click a sphere to take hold of
//! it, and you drag *out of* it to grow the next one. Everything under a sphere
//! travels with it. There is no separate "add" mode — where you press decides
//! what happens, which is what makes rigging feel like modelling rather than
//! like filling in a form.

use clayspace_model::{Armature, ArmatureModel, NodeIndex, SkinSettings};

use crate::observable::Observable;

/// What a press on the viewport means while rigging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grab {
    /// Nothing under the pointer.
    Empty,
    /// Take hold of this sphere and move it, subtree and all.
    Move(NodeIndex),
    /// Grow a new sphere out of this one.
    Grow(NodeIndex),
    /// Change this sphere's radius.
    Resize(NodeIndex),
}

/// A gesture in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dragging {
    grab: Grab,
    /// Where the pointer was last, in world space.
    last: [f32; 3],
    /// The sphere a grow gesture created, once it has one.
    grown: Option<NodeIndex>,
}

pub struct ArmatureViewModel {
    model: Box<dyn ArmatureModel>,

    tree: Observable<Option<Armature>>,
    selected: Observable<Option<NodeIndex>>,
    /// The sphere under the pointer, which is what the viewport highlights.
    hovered: Observable<Option<NodeIndex>>,
    skin: Observable<SkinSettings>,
    /// Whether new spheres are mirrored as they are added.
    symmetric: Observable<bool>,
    notice: Observable<Option<String>>,

    drag: Option<Dragging>,
    /// The radius a new sphere starts at.
    default_radius: f32,
}

impl ArmatureViewModel {
    pub fn new(model: Box<dyn ArmatureModel>) -> Self {
        let tree = model.armature();
        let skin = model.skin();
        Self {
            model,
            tree: Observable::new(tree),
            selected: Observable::new(None),
            hovered: Observable::new(None),
            skin: Observable::new(skin),
            symmetric: Observable::new(true),
            notice: Observable::new(None),
            drag: None,
            default_radius: 0.15,
        }
    }

    pub fn tree(&self) -> &Observable<Option<Armature>> {
        &self.tree
    }

    pub fn selected(&self) -> &Observable<Option<NodeIndex>> {
        &self.selected
    }

    pub fn hovered(&self) -> &Observable<Option<NodeIndex>> {
        &self.hovered
    }

    pub fn skin(&self) -> &Observable<SkinSettings> {
        &self.skin
    }

    pub fn symmetric(&self) -> &Observable<bool> {
        &self.symmetric
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    pub fn is_rigging(&self) -> bool {
        self.tree.get().is_some()
    }

    pub fn set_symmetric(&mut self, on: bool) {
        self.symmetric.set_if_changed(on);
    }

    pub fn set_skin(&mut self, skin: SkinSettings) {
        if let Err(e) = self.model.set_skin(skin) {
            self.notice.set(Some(e.to_string()));
            return;
        }
        self.skin.set(skin);
        self.refresh();
    }

    /// Re-reads the tree, for when the document underneath changed.
    pub fn refresh(&mut self) {
        let tree = self.model.armature();
        // A selection into a tree that has shrunk is worse than none: the next
        // drag would take hold of whatever now sits at that index.
        let live = tree
            .as_ref()
            .map(|t| t.nodes.len() as NodeIndex)
            .unwrap_or(0);
        if self.selected.get().is_some_and(|i| i >= live) {
            self.selected.set(None);
        }
        if self.hovered.get().is_some_and(|i| i >= live) {
            self.hovered.set(None);
        }
        self.tree.set(tree);
    }

    /// Starts a rig at a point, if there is not one already.
    pub fn begin(&mut self, at: [f32; 3]) {
        match self.model.begin_armature(at, self.default_radius * 2.0) {
            Ok(()) => {
                self.notice.set_if_changed(None);
                self.refresh();
                self.selected.set(Some(0));
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
    }

    /// The sphere a ray meets first, nearest the eye.
    ///
    /// Nearest rather than any: rigs overlap constantly — a shoulder sits
    /// inside a torso — and picking the far one would make a chest impossible
    /// to grab.
    pub fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<NodeIndex> {
        let tree = self.tree.get().clone()?;
        let mut best: Option<(f32, NodeIndex)> = None;
        for (index, sphere) in tree.nodes.iter().enumerate() {
            if let Some(t) = ray_hits_sphere(origin, direction, sphere.position, sphere.radius) {
                if best.is_none_or(|(closest, _)| t < closest) {
                    best = Some((t, index as NodeIndex));
                }
            }
        }
        best.map(|(_, index)| index)
    }

    /// Moves the highlight.
    pub fn hover(&mut self, origin: [f32; 3], direction: [f32; 3]) {
        let found = self.pick(origin, direction);
        self.hovered.set_if_changed(found);
    }

    /// What a press here would do.
    ///
    /// `grow` is the modifier that turns hold-and-move into grow-a-child, so
    /// one pointer covers both without a mode to remember.
    pub fn grab_at(&self, origin: [f32; 3], direction: [f32; 3], grow: bool, resize: bool) -> Grab {
        match self.pick(origin, direction) {
            Some(index) if resize => Grab::Resize(index),
            Some(index) if grow => Grab::Grow(index),
            Some(index) => Grab::Move(index),
            None => Grab::Empty,
        }
    }

    /// Begins a gesture.
    pub fn press(&mut self, grab: Grab, at: [f32; 3]) {
        if let Grab::Empty = grab {
            return;
        }
        if let Grab::Move(index) | Grab::Grow(index) | Grab::Resize(index) = grab {
            self.selected.set(Some(index));
        }
        self.drag = Some(Dragging {
            grab,
            last: at,
            grown: None,
        });
    }

    /// Continues one. `at` is where the pointer is now, in world space.
    pub fn drag(&mut self, at: [f32; 3]) {
        let Some(state) = self.drag else {
            return;
        };
        let delta = [
            at[0] - state.last[0],
            at[1] - state.last[1],
            at[2] - state.last[2],
        ];

        let result = match state.grab {
            Grab::Move(index) => self.model.move_zsphere(index, delta),
            Grab::Resize(index) => {
                // Distance from the sphere's centre, so dragging away thickens
                // it — the same gesture ZBrush uses.
                let tree = self.tree.get().clone();
                let radius = tree
                    .and_then(|t| t.get(index).copied())
                    .map(|node| {
                        let d = (0..3)
                            .map(|axis| (at[axis] - node.position[axis]).powi(2))
                            .sum::<f32>()
                            .sqrt();
                        d.max(0.01)
                    })
                    .unwrap_or(self.default_radius);
                self.model.resize_zsphere(index, radius)
            }
            Grab::Grow(parent) => match state.grown {
                // The child exists; keep dragging it about.
                Some(child) => self.model.move_zsphere(child, delta),
                // First movement of the gesture: this is where the new sphere
                // appears, which is what "drag one out of another" means.
                None => {
                    let mirrored = *self.symmetric.get();
                    match self
                        .model
                        .add_zsphere(parent, at, self.default_radius, mirrored)
                    {
                        Ok(child) => {
                            if let Some(drag) = self.drag.as_mut() {
                                drag.grown = Some(child);
                            }
                            self.selected.set(Some(child));
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
            },
            Grab::Empty => Ok(()),
        };

        if let Err(e) = result {
            self.notice.set(Some(e.to_string()));
        } else {
            self.notice.set_if_changed(None);
        }
        if let Some(drag) = self.drag.as_mut() {
            drag.last = at;
        }
        self.refresh();
    }

    pub fn release(&mut self) {
        self.drag = None;
    }

    /// Removes the selected sphere and everything under it.
    pub fn remove_selected(&mut self) {
        let Some(index) = *self.selected.get() else {
            return;
        };
        match self.model.remove_zsphere(index) {
            Ok(()) => {
                self.notice.set_if_changed(None);
                self.selected.set(None);
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
        self.refresh();
    }

    /// Hangs the selection off another sphere.
    pub fn reparent_selected(&mut self, new_parent: NodeIndex) {
        let Some(index) = *self.selected.get() else {
            return;
        };
        match self.model.reparent_zsphere(index, new_parent) {
            Ok(()) => {
                self.notice.set_if_changed(None);
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
        self.refresh();
    }
}

/// Where a ray first meets a sphere, or `None`.
fn ray_hits_sphere(
    origin: [f32; 3],
    direction: [f32; 3],
    centre: [f32; 3],
    radius: f32,
) -> Option<f32> {
    let to_centre = [
        origin[0] - centre[0],
        origin[1] - centre[1],
        origin[2] - centre[2],
    ];
    let a: f32 = (0..3).map(|i| direction[i] * direction[i]).sum();
    if a < 1e-9 {
        return None;
    }
    let b: f32 = 2.0 * (0..3).map(|i| to_centre[i] * direction[i]).sum::<f32>();
    let c: f32 = (0..3).map(|i| to_centre[i] * to_centre[i]).sum::<f32>() - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    // The nearer intersection ahead of the eye.
    let near = (-b - root) / (2.0 * a);
    let far = (-b + root) / (2.0 * a);
    [near, far].into_iter().find(|t| *t > 0.0)
}
