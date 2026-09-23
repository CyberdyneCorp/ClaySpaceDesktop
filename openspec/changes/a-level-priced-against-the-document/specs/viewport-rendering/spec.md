## ADDED Requirements

### Requirement: A surface handed back without normals is lit by its own shape
Where the engine hands back a surface that carries no vertex normals, the
viewport SHALL light it with normals derived from its own triangles, each the
area-weighted sum of the faces around the vertex, and SHALL NOT stand one
constant normal in for all of them.

This holds for a mesh layer filled from bare triangles — which is how a
retopology arrives — and for every level of a hierarchy built over such a cage,
which exports a level's normals only where the cage carried its own. Deriving
them is shading data for the engine's surface; the positions and triangles
drawn are still the engine's.

A vertex no triangle reaches SHALL still be given a unit normal, so that no
normal the viewport uploads is zero or not a number.

#### Scenario: A retopology is lit
- **WHEN** a mesh layer is retopologised and drawn
- **THEN** the drawn normals agree with the triangles they light, rather than
  one normal lighting the whole form as a single colour

#### Scenario: A hierarchy over a retopology is lit
- **WHEN** a retopologised layer is taken as a hierarchy, subdivided and drawn
- **THEN** the level's drawn normals agree with its triangles

#### Scenario: A surface that carries its own normals keeps them
- **WHEN** the engine hands back a surface with normals
- **THEN** those normals are the ones drawn
