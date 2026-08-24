//! Reference images, as something the interface can act on.
//!
//! The pixels arrive from outside — reading a PNG is the engine's job and
//! asking for one is the application's — so this holds what was placed rather
//! than fetching it. What it owns is the placement: which plane, how large,
//! how far back, and how strongly it shows through.

use std::path::{Path, PathBuf};

use clayspace_model::{RefPlane, ReferenceImage, ReferenceSettings};

use crate::command::Command;
use crate::observable::Observable;

/// One plane's reference: the picture, where it came from, and how it sits.
#[derive(Debug, Clone, PartialEq)]
struct Placed {
    image: ReferenceImage,
    path: PathBuf,
}

pub struct ReferenceViewModel {
    /// One entry a plane, in `RefPlane::ALL` order.
    placed: [Option<Placed>; RefPlane::ALL.len()],
    settings: Observable<[ReferenceSettings; RefPlane::ALL.len()]>,
    /// Bumped whenever what the viewport should draw changes.
    ///
    /// The pixels are large and the settings are small, but both change what
    /// is on screen, so one counter covers both rather than the viewport
    /// comparing images it would rather not clone.
    revision: u64,
    /// The last refusal, for the status area.
    notice: Observable<Option<String>>,
}

impl Default for ReferenceViewModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceViewModel {
    pub fn new() -> Self {
        Self {
            placed: Default::default(),
            settings: Observable::new([ReferenceSettings::default(); RefPlane::ALL.len()]),
            revision: 0,
            notice: Observable::new(None),
        }
    }

    pub fn settings(&self) -> &Observable<[ReferenceSettings; RefPlane::ALL.len()]> {
        &self.settings
    }

    pub fn settings_for(&self, plane: RefPlane) -> ReferenceSettings {
        self.settings.get()[plane as usize]
    }

    pub fn image(&self, plane: RefPlane) -> Option<&ReferenceImage> {
        self.placed[plane as usize].as_ref().map(|p| &p.image)
    }

    pub fn path(&self, plane: RefPlane) -> Option<&Path> {
        self.placed[plane as usize]
            .as_ref()
            .map(|p| p.path.as_ref())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    /// Whether any plane has a picture on it.
    pub fn any(&self) -> bool {
        self.placed.iter().any(Option::is_some)
    }

    /// Where a plane's quad sits, or `None` where it has no picture.
    ///
    /// The corners come from the domain, which keeps the picture's proportions
    /// — so an image placed on the front plane and the same image placed on the
    /// side plane are the same size, whatever each plane calls its two axes.
    pub fn corners(&self, plane: RefPlane) -> Option<[[f32; 3]; 4]> {
        let image = self.image(plane)?;
        Some(self.settings_for(plane).corners(plane, image.aspect()))
    }

    /// Puts a picture on a plane, or takes one away.
    ///
    /// Called by the application once it has read the file: this layer has no
    /// disk. A plane that already held a picture keeps its placement — the
    /// sculptor lined up the last one and swapping the file is not a reason to
    /// undo that.
    pub fn place(&mut self, plane: RefPlane, placed: Option<(ReferenceImage, PathBuf)>) {
        self.placed[plane as usize] = placed.map(|(image, path)| Placed { image, path });
        self.revision += 1;
        self.notice.set_if_changed(None);
    }

    /// Restores a placement remembered from a previous session.
    pub fn restore(&mut self, plane: RefPlane, settings: ReferenceSettings) {
        let mut all = *self.settings.get();
        all[plane as usize] = settings.sanitized();
        self.settings.set_if_changed(all);
        self.revision += 1;
    }

    /// Reports that a file could not be read.
    pub fn refuse(&mut self, reason: String) {
        self.notice.set(Some(reason));
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetReferenceSettings(plane, settings) => {
                let mut all = *self.settings.get();
                let sanitized = settings.sanitized();
                if all[*plane as usize] == sanitized {
                    return;
                }
                all[*plane as usize] = sanitized;
                self.settings.set(all);
                self.revision += 1;
            }
            Command::ClearReference(plane) => {
                if self.placed[*plane as usize].is_some() {
                    self.place(*plane, None);
                }
            }
            // Loading asks for a file, which happens above this layer; the
            // picture comes back through `place`.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: u32, height: u32) -> ReferenceImage {
        ReferenceImage {
            name: "referência.png".into(),
            width,
            height,
            pixels: vec![255; (width * height * 4) as usize],
        }
    }

    #[test]
    fn a_plane_holds_its_own_picture() {
        let mut vm = ReferenceViewModel::new();
        vm.place(RefPlane::Front, Some((picture(4, 4), "a.png".into())));
        assert!(vm.image(RefPlane::Front).is_some());
        assert!(
            vm.image(RefPlane::Side).is_none(),
            "one file reached every plane"
        );
    }

    #[test]
    fn changing_the_file_keeps_the_placement() {
        // The sculptor lines the reference up against the form; swapping in a
        // corrected scan of the same drawing should not throw that away.
        let mut vm = ReferenceViewModel::new();
        vm.place(RefPlane::Side, Some((picture(4, 4), "a.png".into())));
        vm.dispatch(&Command::SetReferenceSettings(
            RefPlane::Side,
            ReferenceSettings {
                height: 3.5,
                offset: [0.25, -0.5],
                ..ReferenceSettings::default()
            },
        ));
        vm.place(RefPlane::Side, Some((picture(8, 4), "b.png".into())));

        let held = vm.settings_for(RefPlane::Side);
        assert!((held.height - 3.5).abs() < 1e-6);
        assert_eq!(held.offset, [0.25, -0.5]);
    }

    #[test]
    fn the_viewport_is_told_when_anything_it_draws_changes() {
        let mut vm = ReferenceViewModel::new();
        let start = vm.revision();
        vm.place(RefPlane::Top, Some((picture(4, 4), "a.png".into())));
        let placed = vm.revision();
        assert!(placed > start, "a new picture did not reach the viewport");

        vm.dispatch(&Command::SetReferenceSettings(
            RefPlane::Top,
            ReferenceSettings {
                opacity: 0.2,
                ..ReferenceSettings::default()
            },
        ));
        assert!(
            vm.revision() > placed,
            "the opacity changed and the viewport was not told"
        );

        // And an setting written twice is not a change.
        let steady = vm.revision();
        vm.dispatch(&Command::SetReferenceSettings(
            RefPlane::Top,
            ReferenceSettings {
                opacity: 0.2,
                ..ReferenceSettings::default()
            },
        ));
        assert_eq!(vm.revision(), steady, "an unchanged setting redrew anyway");
    }

    #[test]
    fn a_setting_the_viewport_cannot_draw_is_clamped_here() {
        let mut vm = ReferenceViewModel::new();
        vm.dispatch(&Command::SetReferenceSettings(
            RefPlane::Front,
            ReferenceSettings {
                opacity: 4.0,
                height: 0.0,
                ..ReferenceSettings::default()
            },
        ));
        let held = vm.settings_for(RefPlane::Front);
        assert!(held.opacity <= 1.0);
        assert!(held.height > 0.0, "a reference with no size is invisible");
    }

    #[test]
    fn the_quad_keeps_the_pictures_proportions() {
        let mut vm = ReferenceViewModel::new();
        assert!(
            vm.corners(RefPlane::Front).is_none(),
            "a plane with no picture was placed anyway"
        );
        vm.place(RefPlane::Front, Some((picture(200, 100), "a.png".into())));
        let corners = vm.corners(RefPlane::Front).expect("a placed picture");
        let width = corners[1][0] - corners[0][0];
        let height = corners[3][1] - corners[0][1];
        assert!(
            (width / height - 2.0).abs() < 1e-4,
            "a 2:1 drawing was placed {width} by {height}"
        );
    }

    #[test]
    fn clearing_a_plane_leaves_the_others() {
        let mut vm = ReferenceViewModel::new();
        vm.place(RefPlane::Front, Some((picture(4, 4), "a.png".into())));
        vm.place(RefPlane::Top, Some((picture(4, 4), "b.png".into())));
        vm.dispatch(&Command::ClearReference(RefPlane::Front));
        assert!(vm.image(RefPlane::Front).is_none());
        assert!(vm.image(RefPlane::Top).is_some());
        assert!(vm.any());
    }
}
