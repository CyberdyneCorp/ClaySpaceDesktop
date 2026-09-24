## 1. Lay down what the preview showed

- [x] 1.1 Record one dab per live segment (brush and unreflected centre) in `LiveDabs`, replacing the held samples.
- [x] 1.2 `relax_dabs`: sample the dabs' region once, relax it per dab in order, place it as one Replace item; called per mirror from `close_live_gesture`.
- [x] 1.3 `live_relax_params`: one source for a dab's relax, used by the preview and the release.

## 2. Hold it

- [x] 2.1 `every_dab_of_a_held_gesture_survives_the_release`: twelve dabs in one spot compound in the preview, and the release lands within a tenth of what the preview showed.
