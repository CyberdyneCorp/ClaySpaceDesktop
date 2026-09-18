## ADDED Requirements

### Requirement: The application builds on macOS and Linux from one command
A clean checkout with submodules initialized SHALL build on macOS and on Linux with a single `cargo build`, which SHALL drive the engine's CMake build as part of it. No manual pre-build step SHALL be required beyond installing the stated prerequisites.

#### Scenario: One command from a clean checkout
- **WHEN** a clean checkout with submodules is built on a machine meeting the prerequisites
- **THEN** `cargo build` produces a runnable application without further steps

#### Scenario: Prerequisites are stated and checked
- **WHEN** a prerequisite is missing
- **THEN** the build fails before compiling, naming the prerequisite and its minimum version

### Requirement: Acceleration features are selectable at build time
The build SHALL expose features selecting which engine backends are compiled: Metal on macOS, CUDA and OpenCL on Linux. The CPU backend SHALL always be compiled in and SHALL NOT be disableable. Default features SHALL select the accelerated backends available for the host platform.

#### Scenario: A CPU-only build is possible on any platform
- **WHEN** the build runs with accelerated features disabled
- **THEN** it succeeds on every supported platform and the application runs on the CPU backend

#### Scenario: Requesting an unavailable backend fails clearly
- **WHEN** the CUDA feature is requested on a machine with no CUDA toolkit
- **THEN** the build fails stating that the toolkit was not found, rather than producing a binary that cannot register the backend

### Requirement: CI covers both platforms and both acceleration paths
Continuous integration SHALL build and test on macOS and Linux, covering a CPU-only configuration and an accelerated configuration on each, and SHALL run the headless bridge suite, the ViewModel suite and the architecture checks on every configuration.

#### Scenario: Every configuration is exercised
- **WHEN** CI runs on a pull request
- **THEN** macOS CPU-only, macOS Metal, Linux CPU-only and Linux accelerated configurations each build and run the test suites

#### Scenario: Tests run without a display
- **WHEN** the test suites run in CI
- **THEN** they complete without requiring a display server or a physical GPU

### Requirement: Specifications are validated in CI
Continuous integration SHALL validate the OpenSpec artifacts strictly on every pull request and push, and SHALL fail on a validation error.

#### Scenario: An invalid spec fails the build
- **WHEN** a specification file is malformed or a requirement lacks a scenario
- **THEN** the specification validation job fails and names the file and the problem

### Requirement: Code quality gates run on every change
CI SHALL run formatting, lint and dependency-audit checks and SHALL fail on violations. The lint configuration SHALL enforce the unsafe-code confinement and the layering rules.

#### Scenario: A layering violation fails CI
- **WHEN** a dependency edge is introduced that violates the layering rules
- **THEN** the architecture check fails, naming the offending edge

### Requirement: Releases are reproducible and identify their engine revision
A release build SHALL record the application version and the exact pinned ClayCore revision it was built against, and SHALL make both visible in the application.

#### Scenario: A build identifies its engine
- **WHEN** the user opens the about or diagnostics view of a released build
- **THEN** it shows the application version and the ClayCore revision and version compiled into it

### Requirement: Distributable bundles are produced for both platforms
The project SHALL produce a macOS application bundle and a Linux distributable, each self-contained with respect to the engine, requiring no separately installed ClayCore.

#### Scenario: A bundle runs on a machine without a toolchain
- **WHEN** a produced bundle is run on a supported machine with no CMake, C++ compiler or CUDA toolkit installed
- **THEN** the application starts and sculpting works on whatever backend that machine offers

### Requirement: Third-party licensing is recorded
The build SHALL produce an attribution manifest covering the engine, its dependencies and the Rust dependency tree, and the application SHALL make it available to the user.

#### Scenario: Attribution is available in the application
- **WHEN** the user opens the licenses view
- **THEN** it lists the bundled components and their licenses, including ClayCore and its own third-party manifest
