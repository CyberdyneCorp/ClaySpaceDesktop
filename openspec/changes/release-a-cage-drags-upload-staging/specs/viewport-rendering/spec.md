## ADDED Requirements

### Requirement: An upload does not wait for a frame to release its staging
A whole upload of the carried mesh layers SHALL hand its writes to the graphics
device when it is made, so the staging it takes is released once the device is
done with it whether or not a frame follows. A frame that uploads and then
cannot acquire the window's image SHALL hand its writes to the device before it
returns.

The device memory figure SHALL count staging that no submission has carried as
held. A wait on the device SHALL release only the staging of writes a
submission carried.

#### Scenario: A drag with no frame leaves no staging behind
- **WHEN** the carried mesh is uploaded a hundred times with no frame drawn between the uploads
- **AND** the device is waited on
- **THEN** the device memory figure counts no staging

#### Scenario: An unsubmitted write is still counted after a wait
- **WHEN** a buffer is written, the device is waited on, and no submission was made
- **THEN** the device memory figure still counts the write's staging
