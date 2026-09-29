//! How a layer's UV layout is drawn, as the UV ViewModel holds it.
//!
//! The display is a choice the sculptor makes once and the viewport honours
//! only where there is a layout to draw. These pin both halves: the choice is
//! kept, and it reaches the viewport only while the active subtool carries UVs.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use clayspace_model::{ModelError, Unwrapper, UvDisplay, UvModel, UvResult, UvSettings, UvSource};
use clayspace_vm::{Command, UvViewModel};

/// A document whose active subtool carries UVs exactly when the test says so.
struct Layout {
    carries: Rc<Cell<bool>>,
}

impl UvModel for Layout {
    fn can_unwrap(&self) -> Result<(), String> {
        Ok(())
    }

    fn uv_source(&mut self) -> Result<UvSource, ModelError> {
        Err(ModelError::engine("not under test"))
    }

    fn record_uv(&mut self, _: &UvResult) -> Result<(), ModelError> {
        Ok(())
    }

    fn active_layer_carries_uvs(&mut self) -> bool {
        self.carries.get()
    }
}

struct NoUnwrap;

impl Unwrapper for NoUnwrap {
    fn run(
        &self,
        _: &UvSource,
        _: UvSettings,
        _: &dyn Fn(f32, &str),
        _: &dyn Fn() -> bool,
    ) -> Result<UvResult, String> {
        Err("not under test".into())
    }
}

fn view_model() -> (UvViewModel, Rc<Cell<bool>>) {
    let carries = Rc::new(Cell::new(false));
    let vm = UvViewModel::new(
        Box::new(Layout {
            carries: carries.clone(),
        }),
        Arc::new(NoUnwrap),
    );
    (vm, carries)
}

#[test]
fn the_uv_display_is_off_until_asked_for() {
    let (mut vm, carries) = view_model();
    carries.set(true);
    vm.refresh();
    assert_eq!(*vm.display().get(), UvDisplay::Off);
    assert_eq!(vm.shown_display(), UvDisplay::Off);
}

#[test]
fn a_chosen_display_is_shown_only_on_a_layer_carrying_uvs() {
    let (mut vm, carries) = view_model();
    vm.refresh();
    vm.dispatch(&Command::SetUvDisplay(UvDisplay::Checker));
    assert_eq!(*vm.display().get(), UvDisplay::Checker);
    assert!(!*vm.carries_uvs().get());
    assert_eq!(
        vm.shown_display(),
        UvDisplay::Off,
        "a checker was drawn on a layer with no layout"
    );

    // A retopology with UVs lands and becomes the active subtool.
    carries.set(true);
    vm.refresh();
    assert!(*vm.carries_uvs().get());
    assert_eq!(vm.shown_display(), UvDisplay::Checker);

    vm.dispatch(&Command::SetUvDisplay(UvDisplay::Islands));
    assert_eq!(vm.shown_display(), UvDisplay::Islands);

    // Choosing a layer without one hides the display and keeps the choice.
    carries.set(false);
    vm.refresh();
    assert_eq!(vm.shown_display(), UvDisplay::Off);
    assert_eq!(*vm.display().get(), UvDisplay::Islands);
}

#[test]
fn choosing_a_uv_display_does_not_touch_the_document() {
    for display in UvDisplay::ALL {
        let command = Command::SetUvDisplay(display);
        assert!(!command.touches_document(), "{command:?}");
        assert!(!command.changes_the_document(), "{command:?}");
    }
}
