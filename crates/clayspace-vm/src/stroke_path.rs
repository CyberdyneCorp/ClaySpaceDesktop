//! A stroke being drawn: the path it has made, and where on it the next stamp
//! is owed.
//!
//! A gesture reaches the model in segments, so the sculptor watches the clay
//! move under the pointer rather than when it comes up. The engine resolves
//! each call on its own — it lays the first stamp at the call's first sample,
//! spaces the rest by arc length from there, and runs its lazy mouse from that
//! same sample — so two things that belong to the *gesture* have to be carried
//! across segments here, or they restart at every joint:
//!
//! * **Where the next stamp is owed.** A segment that began at its own first
//!   new sample left the stretch between the last stamp and that sample
//!   unstamped. Delivered densely, by a pointer, the stretch was shorter than
//!   a stamp and nobody saw it. Delivered sparsely, by an agent, every
//!   segment was a single far-off sample, and the stroke came out as one bead
//!   per sample (#178). So a segment starts from the last sample already sent
//!   and is trimmed to begin exactly where the next stamp falls, and the
//!   stamps land where the whole gesture resolved at once would put them.
//!
//! * **The lazy mouse** (Suavização). Run by the engine per call, the trailing
//!   point started again at every segment's first sample, and a segment is
//!   only three stamps long, so the steadying had no path to act on: a zigzag
//!   drawn at 0 and at 0.5 came out the same. It is run here instead, once,
//!   over the whole gesture, with the engine's own first-order lag.

use clayspace_model::GestureSample;

/// How much slack arc-length spacing allows before counting a stamp, matching
/// the engine's own: the same path summed from sparse and from dense samples
/// lands a few ulps apart, and the stamp count must not flip on it.
const SPACING_SLACK: f32 = 1e-3;

/// A nominal time step between samples. Wall-clock is not available here;
/// the engine uses time only to order the samples and to drive taper.
const SAMPLE_INTERVAL: f32 = 0.008;

/// A stroke being drawn.
#[derive(Debug, Default)]
pub(crate) struct ActiveStroke {
    /// The path as the stamps will follow it: steadied, where the stroke is
    /// steadied at all.
    samples: Vec<GestureSample>,
    next_time: f32,
    /// How many samples have already been sent to the model.
    applied: usize,
    /// Arc length travelled since the last segment was sent.
    travelled: f32,
    /// Lazy-mouse lag, 0 to follow the pointer exactly.
    lag: f32,
    /// Distance from the last stamp laid down to the last sample sent, or
    /// `None` while nothing has been stamped.
    since_stamp: Option<f32>,
}

impl ActiveStroke {
    /// A stroke steadied by `lag` — the brush's Suavização, or zero for a verb
    /// that must follow the pointer exactly.
    pub(crate) fn new(lag: f32) -> Self {
        Self {
            lag: lag.clamp(0.0, 0.95),
            ..Self::default()
        }
    }

    pub(crate) fn push(&mut self, position: [f32; 3], pressure: f32) {
        // The engine's steady stroke: the emission point trails the pointer
        // by a first-order lag, starting where the pointer went down.
        let position = match self.samples.last() {
            Some(previous) => {
                let steadied = lerp3(position, previous.position, self.lag);
                self.travelled += distance(previous.position, steadied);
                steadied
            }
            None => position,
        };
        self.samples.push(GestureSample {
            position,
            pressure: pressure.clamp(0.0, 1.0),
            time: self.next_time,
        });
        self.next_time += SAMPLE_INTERVAL;
    }

    /// Whether `travelled` has reached `threshold` with something unsent.
    pub(crate) fn has_travelled(&self, threshold: f32) -> bool {
        self.applied < self.samples.len() && self.travelled >= threshold
    }

    /// Every sample since the press, which is what a replayed gesture needs.
    pub(crate) fn whole(&self) -> &[GestureSample] {
        &self.samples
    }

    /// The samples not yet sent, and the last one that was — a displacement
    /// needs somewhere to start from, and re-sending it costs nothing: a drag
    /// moves the surface from that point, it does not deposit at it.
    pub(crate) fn since_last_sent(&self) -> &[GestureSample] {
        let applied = self.applied.min(self.samples.len());
        &self.samples[applied.saturating_sub(1)..]
    }

    /// The next stamping segment, starting where the next stamp is owed.
    ///
    /// `None` when the path has not yet reached that point: there is no stamp
    /// to lay down, and sending the stretch anyway would put one early.
    pub(crate) fn stamping_segment(&self, gap: f32) -> Option<Vec<GestureSample>> {
        let Some(since) = self.since_stamp else {
            // Nothing stamped yet: the first stamp belongs on the press.
            return (!self.samples.is_empty()).then(|| self.samples.clone());
        };
        let path = self.since_last_sent();
        trimmed(path, (gap - since).max(0.0))
    }

    /// Records a segment as sent, whether or not the engine accepted it:
    /// re-sending one the engine refused would refuse again every frame, and
    /// re-sending one it accepted would deposit it twice.
    pub(crate) fn mark_applied(&mut self) {
        self.applied = self.samples.len();
        self.travelled = 0.0;
    }

    /// Records a stamping segment as sent, and where its last stamp fell.
    pub(crate) fn mark_stamped(&mut self, segment: &[GestureSample], gap: f32) {
        self.since_stamp = Some(beyond_last_stamp(arc_length(segment), gap));
        self.mark_applied();
    }
}

/// How far past its last stamp a path of `length` ends, spaced the way the
/// engine spaces a call: a stamp at the start and one every `gap` after it.
fn beyond_last_stamp(length: f32, gap: f32) -> f32 {
    if gap <= 0.0 || length < gap {
        return length.max(0.0);
    }
    let stamps = (length / gap + SPACING_SLACK).floor();
    (length - stamps * gap).max(0.0)
}

/// `path` with its first `skip` of arc length cut away, or `None` when the
/// path is shorter than that.
fn trimmed(path: &[GestureSample], skip: f32) -> Option<Vec<GestureSample>> {
    let mut walked = 0.0;
    for (index, pair) in path.windows(2).enumerate() {
        let step = distance(pair[0].position, pair[1].position);
        if step > 0.0 && walked + step >= skip {
            let t = ((skip - walked) / step).clamp(0.0, 1.0);
            let mut segment = vec![lerp_sample(pair[0], pair[1], t)];
            segment.extend_from_slice(&path[index + 1..]);
            return Some(segment);
        }
        walked += step;
    }
    // A path that has not moved still owes its first stamp, but only when no
    // distance is owed before it.
    (skip <= 0.0 && !path.is_empty()).then(|| path.to_vec())
}

fn arc_length(path: &[GestureSample]) -> f32 {
    path.windows(2)
        .map(|pair| distance(pair[0].position, pair[1].position))
        .sum()
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3)
        .map(|axis| (a[axis] - b[axis]).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// `a` moved toward `b` by `t`.
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|axis| a[axis] + (b[axis] - a[axis]) * t)
}

fn lerp_sample(a: GestureSample, b: GestureSample, t: f32) -> GestureSample {
    GestureSample {
        position: lerp3(a.position, b.position, t),
        pressure: a.pressure + (b.pressure - a.pressure) * t,
        time: a.time + (b.time - a.time) * t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> [f32; 3] {
        [x, 0.0, 0.0]
    }

    /// Where the engine would put the stamps of one call over `path`.
    fn stamps_of(path: &[GestureSample], gap: f32) -> Vec<f32> {
        let length = arc_length(path);
        let count = if length < gap {
            1
        } else {
            (length / gap + SPACING_SLACK).floor() as usize + 1
        };
        let start = path[0].position[0];
        (0..count).map(|i| start + i as f32 * gap).collect()
    }

    /// Drives a stroke along x, one segment per sample, and collects the
    /// stamps every segment would lay down.
    fn segmented(xs: &[f32], gap: f32) -> Vec<f32> {
        let mut stroke = ActiveStroke::new(0.0);
        let mut stamps = Vec::new();
        for &x in xs {
            stroke.push(at(x), 1.0);
            if let Some(segment) = stroke.stamping_segment(gap) {
                stamps.extend(stamps_of(&segment, gap));
                stroke.mark_stamped(&segment, gap);
            }
        }
        stamps
    }

    #[test]
    fn sparse_segments_stamp_where_the_whole_stroke_would() {
        // Four samples a long way apart, each sent on its own — what an agent
        // stroke is. Each segment used to start at its own sample and lay a
        // single stamp there: four beads a unit apart.
        let gap = 0.1;
        let stamps = segmented(&[0.0, 1.0, 2.0, 3.0], gap);
        let whole: Vec<f32> = (0..=30).map(|i| i as f32 * gap).collect();
        assert_eq!(stamps.len(), whole.len(), "stamps: {stamps:?}");
        for (got, want) in stamps.iter().zip(&whole) {
            assert!(
                (got - want).abs() < 1e-3,
                "a stamp at {got}, owed at {want}"
            );
        }
    }

    #[test]
    fn a_joint_is_neither_a_gap_nor_a_double_stamp() {
        // Segments whose ends fall between stamps, at uneven lengths.
        let gap = 0.07;
        let stamps = segmented(&[0.0, 0.05, 0.31, 0.33, 0.9, 0.95, 1.4], gap);
        for pair in stamps.windows(2) {
            let spacing = pair[1] - pair[0];
            assert!(
                (spacing - gap).abs() < 1e-3,
                "two stamps {spacing} apart at a joint, where the stroke spaces them {gap}"
            );
        }
    }

    #[test]
    fn a_segment_short_of_the_next_stamp_sends_nothing() {
        let mut stroke = ActiveStroke::new(0.0);
        stroke.push(at(0.0), 1.0);
        let first = stroke.stamping_segment(0.1).expect("the press is a stamp");
        stroke.mark_stamped(&first, 0.1);
        stroke.push(at(0.04), 1.0);
        assert!(
            stroke.stamping_segment(0.1).is_none(),
            "a stamp was owed 0.1 along and the path had gone 0.04"
        );
    }

    fn zigzag(lag: f32) -> f32 {
        let mut stroke = ActiveStroke::new(lag);
        for i in 0..40 {
            let y = if i % 2 == 0 { 0.05 } else { -0.05 };
            stroke.push([i as f32 * 0.02, y, 0.0], 1.0);
        }
        let ys: Vec<f32> = stroke.whole()[10..].iter().map(|s| s.position[1]).collect();
        ys.iter().fold(0.0f32, |m, y| m.max(y.abs()))
    }

    #[test]
    fn smoothing_damps_a_shaky_path() {
        let raw = zigzag(0.0);
        let steadied = zigzag(0.5);
        assert!((raw - 0.05).abs() < 1e-6, "an unsteadied path moved: {raw}");
        assert!(
            steadied < raw * 0.5,
            "Suavização 0.5 left a zigzag of {raw} at {steadied}"
        );
    }

    #[test]
    fn smoothing_is_carried_across_segments() {
        // The lag is the gesture's, not the segment's: sending in between must
        // not let the trailing point catch up with the pointer.
        let mut sent = ActiveStroke::new(0.5);
        let mut held = ActiveStroke::new(0.5);
        for i in 0..12 {
            let position = [i as f32 * 0.05, 0.0, 0.0];
            sent.push(position, 1.0);
            held.push(position, 1.0);
            if let Some(segment) = sent.stamping_segment(0.02) {
                sent.mark_stamped(&segment, 0.02);
            }
        }
        assert_eq!(sent.whole(), held.whole());
    }
}
