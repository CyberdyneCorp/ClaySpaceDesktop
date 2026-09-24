## ADDED Requirements

### Requirement: A circle tube has the authored radius
A circle curve's evaluated surface SHALL be within 10% of twice its authored radius in diameter at the middle of a straight span, for supported radii from 0.02 through 0.5 world units. Doubling guide point density SHALL NOT change the measured half-width by more than 5% of the authored radius. Along a straight span, measured half-width SHALL stay within 10% of the authored radius under each join mode.

#### Scenario: A straight tube is sampled at several densities
- **WHEN** a circle tube of radius 0.02, 0.1 or 0.5 is placed on a straight guide with 2, 10 or 50 points
- **THEN** its mid-span field zero crossing is within 10% of the requested radius
- **AND** the measured width differs by at most 5% of the requested radius across densities

#### Scenario: A straight tube is sampled along its length
- **WHEN** a circle tube uses Corners, Through or Rounded joins
- **THEN** the half-width at one-quarter, one-half and three-quarters of its straight guide is within 10% of the requested radius

### Requirement: Dense sectioned tubes have no longitudinal corner spikes
For square, hexagon and triangle section profiles, the evaluated half-width in a fixed radial direction SHALL vary by less than 0.01 world units between one-quarter, one-half and three-quarters of a straight 50-point fixture of radius 0.1.

#### Scenario: A dense non-circle tube is sampled around its section
- **WHEN** a square, hexagon or triangle tube is placed on a straight 50-point guide
- **THEN** eight radial directions sampled at three positions show no longitudinal spike larger than 0.01 world units
