# Design

Two of the changes here contradict requirements the design system already
states. Both contradictions are deliberate, and this is where they are argued
rather than quietly written over.

## 1. The accent stops meaning "the active brush" and starts meaning "state"

`add-clayspace-desktop` wrote:

> `#D9744A` SHALL indicate the active brush and the active tool state. It SHALL
> NOT be used for … ordinary selection …
>
> #### Scenario: Selection is not accented
> - **WHEN** a layer or scene entry is selected
> - **THEN** it is indicated by surface tone and weight, not by the accent color

The rule was right about what it was defending — a panel full of orange is
noise — and wrong about its instrument. It banned the accent from selection and
left tone as the only carrier, and tone alone cannot do the job here: `panel`
(`#2E3238`) to `raised` (`#3A3E45`) is a 3.5% step in relative luminance, and
it is being asked to answer "which of these four layers am I sculpting?" from
across a desk.

The narrower rule that keeps the defence and drops the failure is about *area*
rather than about *which states*:

> The accent SHALL be used at the scale of a rail, a mark or a label, and SHALL
> NOT fill a panel, a row or a card.

A 2 px rail down the leading edge of one row is not "a panel full of orange".
It is the smallest mark that reads at a glance, it is what the approved concept
draws, and it composes with the tone step rather than replacing it — which is
what keeps the accessibility scenario below intact.

**What does not change:** state is still never carried by hue alone. The active
layer is raised *and* railed *and* set in primary rather than dimmed text; the
active brush is raised *and* railed *and* labelled in the accent *and* ringed.
Cover the hue and every one of them is still identifiable, which is the
scenario `State is not conveyed by color alone` exists to hold and which this
change re-states rather than relaxes.

## 2. Sliders gain a filled range

`add-clayspace-desktop` wrote:

> #### Scenario: Sliders read as a track and a position
> - **WHEN** a slider is displayed
> - **THEN** it is drawn as a thin track with a position marker and its numeric
>   value, without a raised handle or a filled decorative bar

The operative word is **decorative**. The scenario was written against the
inspector's sliders, where a filled bar would be ornament on a value that is
already printed beside it in digits.

The options bar is a different case, and it did not exist in its present form
when the rule was written. `Intensity`, `Size` and `Flow` are the three
controls a sculptor adjusts mid-stroke, by dragging, without reading. There the
fill is not decoration but the control's *state*: it says how far into its
range the value sits, which is the one thing a digit cannot say without being
read. A hairline track says nothing until the eye has found a 4 px knob on it.

So the requirement is modified rather than dropped, and the distinction it now
draws is fill-as-position versus fill-as-ornament:

- the fill spans track-start to knob and stops there — it is the traversed
  range, not a bar that brightens the control;
- the knob stays restrained at rest and lifts only under the pointer, so the
  row is quiet when nobody is addressing it;
- there is still exactly one numeric value, set monospaced, right-aligned.

One widget rather than a treatment applied per call site: `sculpt_slider` is
the only slider in the shell, and `slider`/`slider_named` become thin wrappers
so the thirty-odd existing call sites are unchanged.

## 3. Why the grid moves when the viewport does

`GRID_MINOR` and `GRID_AXIS` were chosen to sit a particular distance above
`#23262B`. Dropping the viewport to `#1B1E22` without touching them widens that
distance, and the grid — which the guide wants *less* CAD-like — would get
more prominent as an accident of a change that was about the shell.

Both tones therefore drop by the same 8/8/9 the viewport drops, preserving the
step they were tuned to. The test asserts the relationship (`viewport <
grid minor < grid axis`, and the axis lines far below the foreground) rather
than the hex values, so the next retune has to keep the relationship rather
than re-derive it.

The two tones stop being shared with the shell's `panel` and `raised` in the
same move. They were never the same thing: one is a line drawn on the
viewport's ground, the other a surface in the chrome. That they held equal
values was a coincidence of there being one ground.

## 4. The four-step ladder, and what fixes it in place

```
viewport  #1B1E22   the sculpt's ground; the renderer's clear colour
ground    #23262B   the application shell
panel     #2E3238   a panel on the shell
raised    #3A3E45   a selected row, a pressed control
```

`surfaces_are_distinguishable_from_one_another` already walks the ladder in
pairs and asserts each step rises. `viewport` is prepended to it. The contrast
floors are re-run against `viewport` as a surface in its own right, because the
viewport bar's chips and the tool-status line are drawn on it.
