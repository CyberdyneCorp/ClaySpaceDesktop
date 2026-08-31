# Tasks

## 1. Focus mode

- [x] 1.1 Check `Tab` is free before binding it — it was
- [x] 1.2 `Action::ToggleFocus`, handled in the composition root rather than as
      a command, like Sair beside it
- [x] 1.3 Hide the rail, the options bar, the representation bar, both
      inspectors, the shelf and the status area; keep the menu bar and the view
      presets
- [x] 1.4 Offer it from the Janela menu too, with the key beside it
- [x] 1.5 Keep it out of the store: an application opening with its panels gone
      would look broken

## 2. The brush readout

- [x] 2.1 The ball, the mark, the name, the representation, and the three
      numbers the options bar carries
- [x] 2.2 The opposite corner from the transform readout, so the two never
      stack

## 3. The stroke's settings on one bar

- [x] 3.1 Move the smoothing out of the brush-controls section
- [x] 3.2 Move the symmetry axes out of the left region, and drop the section
      that held nothing else
- [x] 3.3 Try the falloff shape too, look at the result, and put it back: the
      bar overflowed and scrolled the colour swatch out of view
- [x] 3.4 Narrow the sliders to fit a fourth
- [x] 3.5 Give an engaged axis the soft accent rather than a raised grey
- [x] 3.6 Use a dim label for the symmetry heading — `numeric` is a monospaced
      face for digits and came out heavier than every other heading on the bar

## 4. Favourites

- [x] 4.1 A `ShelfFilter` enum, since the filter is no longer one of four
      representations
- [x] 4.2 Star and unstar from the brush's own menu
- [x] 4.3 Store them, with a stable key per brush
- [x] 4.4 Say how to star one where the filter is chosen and nothing is

## 5. The viewport profile

- [x] 5.1 Store it, with a stable key, and open the governor on it
- [x] 5.2 Load only a tier this build knows, as the locale does

## 6. Tests

- [x] 6.1 Focus mode draws the brush readout and an ordinary frame does not
- [x] 6.2 Focus does not disturb which regions were put away
- [x] 6.3 `Tab` is bound to it
- [x] 6.4 Favourites and the profile round-trip; an unknown brush costs its own
      line; an unknown tier is not a silent preference
- [x] 6.5 The star filter lists what was starred, including a brush from
      another representation, and nothing else
- [x] 6.6 Keep the capture harness in step with the composition root on focus
- [x] 6.7 Add `Tab` to the keycode sweep that asserts every bound action is
      reachable — it caught the binding before CI did
- [x] 6.8 Point the edge-profile test back at the right panel after the move
      was reverted

## 7. Crossing a layer from its own row

- [x] 7.1 Offer the crossings *that layer's* representation has, from its own
      menu — the representation bar speaks for the active layer, and a sculptor
      looking at a stack means the row they opened
- [x] 7.2 Make the row's layer active first, since the conversion acts on the
      active one
- [x] 7.3 Aim it in place, and open the panel rather than converting on the
      click: a crossing into cells needs a size chosen and one over the budget
      is refused
- [x] 7.4 An ellipsis on the entry, which is what says a dialog follows
- [x] 7.5 Move `DELETE_ENTRY`, measured rather than reasoned

## 8. The capture harness's missing accents

- [x] 8.1 Establish it is the harness and not the application: a glyph reaches
      the font atlas in the pass that first lays it out, and a menu is laid out
      in a pass whose output was discarded
- [x] 8.2 Apply every pass's texture deltas rather than the first and the last
- [x] 8.3 Confirm "Mostrar só esta" keeps its accent

## 9. Verification

- [x] 9.1 Look at focus mode, at the bar, and at the layer's menu
- [x] 9.2 `just check`
