## ADDED Requirements

### Requirement: The mask an agent reads is what is frozen
The mask an agent is shown SHALL answer whether anything is frozen on the
active subtool, which is the question a caller about to make a stroke is
asking. It SHALL NOT answer whether the layer carries a mask field: a document
has no verb for detaching one, so an emptied mask stays attached and the two
answers differ exactly after a clear — where an agent was told a region was
still protected that protects nothing.

#### Scenario: A cleared mask reads as nothing frozen
- **WHEN** an agent clears the mask and reads the session state
- **THEN** the mask reads as not present

#### Scenario: A painted mask reads as frozen
- **WHEN** an agent paints a mask and reads the session state
- **THEN** the mask reads as present
