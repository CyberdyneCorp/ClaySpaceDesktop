## ADDED Requirements

### Requirement: User-visible notices follow the selected locale
Every visible label, refusal, diagnostics heading and export warning SHALL be supplied by the locale table or formatted from a localized template. An en-US session SHALL not display Portuguese fallback text. A raw engine ABI identifier SHALL not be displayed as the user-facing reason for a refusal.

#### Scenario: A model refuses an action in English
- **WHEN** an en-US session triggers a model or engine refusal
- **THEN** the visible message SHALL be English and SHALL not include a raw `clay_` ABI symbol

#### Scenario: An export warning follows the active locale
- **WHEN** the export panel predicts or observes a mesh warning
- **THEN** it SHALL format the warning from its typed cause in the selected locale

### Requirement: Hierarchy has one visible name
The subdivision representation SHALL be called Hierarchy in visible English wording and by its translated equivalent in other shipped locales.

#### Scenario: Hierarchy appears in a command result
- **WHEN** a hierarchy is named in a panel, notice or agent-facing description
- **THEN** the panel or notice SHALL use the locale's hierarchy term, while protocol metadata SHALL use the English term "Hierarchy"
