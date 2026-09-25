//! Voxel Move is held until release because the grid's grab composes destructively.
//! The ViewModel is the seam that must keep pointer updates from becoming grabs.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use clayspace_vm::{Command, SculptViewModel};

const CELL: f32 = 0.05;
type OccupiedCells = Vec<([i32; 3], i32)>;

fn slab() -> SharedDocument {
    let policy = BackendPolicy::discover(None).expect("backends");
    let mut document = ClayDocument::new(policy).expect("document");
    document.add_voxel_layer("Voxels", CELL).expect("grid");
    let brush = BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    for step in 0..25 {
        let t = step as f32 / 24.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [(t - 0.5) * 1.8, 0.0, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("deposit");
    }
    SharedDocument::new(document)
}

fn cells(document: &SharedDocument) -> OccupiedCells {
    document.with(|document| {
        let (_, reader) = document.document().voxel_reader("Voxels").expect("grid");
        let Some((min, max)) = reader.bounds().expect("bounds") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    if let Some(value) = reader.get([x, y, z]).expect("cell") {
                        out.push(([x, y, z], value));
                    }
                }
            }
        }
        out
    })
}

fn surface(document: &SharedDocument) -> f32 {
    document.with(|document| {
        let (_, reader) = document.document().voxel_reader("Voxels").expect("grid");
        let mut top = i32::MIN;
        for y in -40..40 {
            if reader.get([0, y, 0]).expect("cell").is_some() {
                top = top.max(y);
            }
        }
        (top + 1) as f32 * CELL
    })
}

fn drag(document: &SharedDocument, updates: usize, cancel: bool) -> (OccupiedCells, OccupiedCells) {
    let mut vm = SculptViewModel::new(Box::new(document.clone()));
    vm.dispatch(Command::SelectTool(ToolKind::Mover))
        .expect("select Move");
    vm.dispatch(Command::SetBrushSize(0.4)).expect("size");
    vm.dispatch(Command::SetBrushIntensity(1.0))
        .expect("intensity");
    vm.dispatch(Command::SetBrushFlow(1.0)).expect("flow");
    let from = surface(document);
    vm.dispatch(Command::BeginStroke {
        position: [0.0, from, 0.0],
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    for step in 1..=updates {
        vm.dispatch(Command::ContinueStroke {
            position: [0.0, from + 0.3 * step as f32 / updates as f32, 0.0],
            pressure: 1.0,
        })
        .expect("continue");
    }
    let during = cells(document);
    vm.dispatch(if cancel {
        Command::CancelStroke
    } else {
        Command::EndStroke
    })
    .expect("finish");
    (during, cells(document))
}

#[test]
fn voxel_move_is_one_captured_gesture() {
    let segmented = slab();
    let whole = slab();
    let rest = cells(&segmented);
    let (during, after) = drag(&segmented, 8, false);
    let (_, once) = drag(&whole, 1, false);
    assert_eq!(
        during, rest,
        "pointer updates applied destructive grabs before release"
    );
    assert_ne!(after, rest, "the Move gesture changed no cells");
    assert_eq!(
        after, once,
        "segmentation changed the final grid cell for cell"
    );

    let cancelled = slab();
    let before_cancel = cells(&cancelled);
    let (_, after_cancel) = drag(&cancelled, 8, true);
    assert_eq!(
        after_cancel, before_cancel,
        "cancel left a captured grab behind"
    );
}
