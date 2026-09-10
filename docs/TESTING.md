# Roldex Studio Testing Strategy

Roldex testing should match the feature being built.

## Structural checks

Use live Studio queries to verify expected Instances, classes, parents, properties, attributes, script placement, remotes, effects, sounds and other place state.

## Runtime checks

Use Studio run/play/multiplayer tests for code that depends on runtime behavior. Inspect Studio Output and repair errors before completion.

## Interaction checks

For player-facing flows, use bounded virtual input when supported by Studio: keyboard, mouse, pointer and text actions. This can exercise the experience's own controls and UI, including representative Tool/ability activation and animation-trigger flows.

## Visual checks

Capture the Studio viewport and inspect it with the vision model for visible evidence such as UI clipping, scale, readability, composition, lighting, misplaced objects and expected visible interaction states.

## Device checks

For responsive interfaces, use Studio device simulation with representative phone/tablet/desktop dimensions and orientation before visual capture.

## Verification loop

A substantial task should follow the relevant subset of:

`inspect -> build/edit -> structural verify -> runtime test -> interaction scenario -> viewport capture -> vision review -> repair -> repeat`

Do not infer hidden correctness from a screenshot and do not infer visual quality from an object hierarchy alone.
