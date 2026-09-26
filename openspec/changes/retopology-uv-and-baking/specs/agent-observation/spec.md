## MODIFIED Requirements

### Requirement: The application can be asked to be quiet
An agent SHALL be able to wait for the session to reach a settled state — no
pending re-mesh, no running job, no queued maintenance — with a bound on how
long it will wait.

A job that runs off the interface thread — a retopology, a UV layout, a
conform, a bake — SHALL count as running from the moment it starts until its
result has been published or discarded. It SHALL be listed with its progress
fraction wherever outstanding work is reported, and waiting SHALL collect and
publish its result rather than depend on the window drawing a frame.

Where the bound is reached, the answer SHALL name what is still running rather
than reporting only that time ran out.

#### Scenario: Waiting for quiet
- **WHEN** an agent asks the session to settle after an import
- **THEN** the answer returns once the import, its meshing and its maintenance
  are done

#### Scenario: A bound that is reached names the work
- **WHEN** the wait's bound is reached with work still running
- **THEN** the answer names what is running and how far along it is

#### Scenario: A retopology is outstanding until it lands
- **WHEN** an agent starts a retopology and waits
- **THEN** the session is not reported quiet while the job runs, the job is
  listed with its fraction, and the wait returns quiet once the result is
  published or discarded
