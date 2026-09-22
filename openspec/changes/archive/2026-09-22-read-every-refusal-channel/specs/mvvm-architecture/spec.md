## ADDED Requirements

### Requirement: Every notice channel a ViewModel owns is one the shell reads
A ViewModel that can refuse SHALL carry its refusal on an observable channel,
and the composition root SHALL read every such channel: both to draw it on the
interface's one "why that did not happen" line and to compare it either side of
a command for the agent door.

The channels SHALL be named in one list rather than in each reader. A reader
that samples a channel's count before a command and reads its words afterwards
SHALL take both from that one list, so a channel present in one reading and
absent from the other is not expressible.

Registering a channel SHALL NOT be a matter of review. Adding a ViewModel that
owns a notice channel without adding it to the list SHALL fail an automated
check that names the channel.

Writing a refusal to the process's error stream SHALL NOT stand in for either
reader.

#### Scenario: A panel's refusal reaches both readers
- **WHEN** a cage, a curve, a boolean or a rig command is refused
- **THEN** the reason appears on the interface's one "why that did not happen"
  line, and the command is answered to the client as a refusal carrying the
  same reason

#### Scenario: A ViewModel added without registering its channel fails a check
- **WHEN** a ViewModel the composition root holds owns a notice channel that no
  reader is named against
- **THEN** an automated check fails, naming the channel that is written and
  never read

#### Scenario: A refusal on any channel is the command's answer
- **WHEN** a command is refused on a channel other than the first
- **THEN** it is still answered as a refusal, with the sentence that channel
  was written
