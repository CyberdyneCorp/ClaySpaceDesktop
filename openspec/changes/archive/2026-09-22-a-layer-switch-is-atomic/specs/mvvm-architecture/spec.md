## ADDED Requirements

### Requirement: One command reaches every ViewModel against one document state
A command SHALL be dispatched to the ViewModels in an order that lets each of
them read a document the command has already reached. Where one ViewModel
changes document state that others read while handling the same command, that
ViewModel SHALL be dispatched to first.

The active layer is the case this exists for: the scene ViewModel is the only
one that moves it, and the sculpting, mask, cage and manipulator ViewModels all
read it while handling the same command. Dispatched after any of them, a
selection leaves each follower set up for the layer that was left, and the
*next* command is the first to see a consistent document — an error that is
always exactly one command behind and therefore reads as a defect in whatever
was done next.

Where the composition root moves the active layer without a command passing
through the ViewModels — a conversion, opening a document, starting a rig — it
SHALL tell the ViewModels that read the active layer to catch up, by name.

#### Scenario: A follower reads the layer the command selected
- **WHEN** a layer-selection command is dispatched
- **THEN** every ViewModel that reads the active layer while handling it reads
  the newly selected layer, not the previous one

#### Scenario: The order is a property of the composition root
- **WHEN** the composition root's dispatch is inspected
- **THEN** the ViewModel that moves the active layer is dispatched to before
  every ViewModel that reads it
