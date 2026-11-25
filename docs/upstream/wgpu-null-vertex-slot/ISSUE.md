# Draft: wgpu 29: a vertex buffer layout with no attributes shifts the slots after it

> **Do not file (checked 2026-11-25).** It is fixed in wgpu 30 (#9351), and wgpu's CONTRIBUTING ("LLMs (AI)") asks that
> issue descriptions be written by the person filing them, so this AI-written text may not be posted there.

Not filed. Found by nsh-webgpu's commands area while porting react-native-webgpu's `NullSlots.spec.ts`.

**Check before filing.** The bug is fixed on wgpu trunk by [#9351](https://github.com/gfx-rs/wgpu/pull/9351)
("Make vertex buffer slots optional", merged 2026-04-15, first released in v30.0.0): wgpu-core now pushes `None`
for an attribute-less layout instead of skipping it. The same repro passes on `wgpu = "=30.0.2"`
(`docs/evidence/integration/wgpu-null-vertex-slot-v30.txt`). So a new issue would be a duplicate; what is left to
ask is a **29.x backport** (29.0.4 is still the 29 line's latest), and that only if the maintainers backport
correctness fixes there. nsh-webgpu itself moves off the bug by moving to wgpu 30 when NSH does.

## Title

wgpu-core 29: an attribute-less `VertexBufferLayout` before a used one makes the used one read the wrong buffer

## Description

In wgpu 29.0.4, `Device::create_render_pipeline` leaves a vertex buffer layout with no attributes out of the HAL
pipeline without renumbering the layouts after it (`wgpu-core/src/device/resource.rs`, `create_render_pipeline`):

```rust
vertex_steps.push(pipeline::VertexStep { stride: vb_state.array_stride, last_stride, mode: vb_state.step_mode });
if vb_state.attributes.is_empty() {
    continue;
}
vertex_buffers.push(hal::VertexBufferLayout { /* ... */ });
```

`vertex_steps` keeps the user's slot numbers (validation and `set_vertex_buffer` use them), but the HAL pipeline's
layouts are now compacted: with `buffers: [no attributes, layout with @location(0)]`, the HAL's buffer 0 is the
second layout. `set_vertex_buffer(1, ..)` binds HAL slot 1, which the pipeline does not read, and the attribute is
fetched from the buffer bound at slot 0.

WebGPU allows the hole (`GPUVertexState.buffers` is `sequence<GPUVertexBufferLayout?>`; a null entry keeps the
indices of the ones after it), so a WebGPU implementation on wgpu (nsh-webgpu, a port of react-native-webgpu)
that maps `null` to an empty layout renders with the wrong data and no error.

## Repro

`docs/upstream/wgpu-null-vertex-slot` (a standalone cargo project, `cargo run`): a full-target triangle whose color
is a vertex attribute. Slot 0: no attributes, bound to a buffer of red. Slot 1: the color attribute, bound to a
buffer of green.

```text
adapter: "Apple M3 Max" (Metal)
control   (slot 0 = green layout):             [0, 255, 0, 255]
with hole (slot 0 = no attributes, slot 1 = green): [255, 0, 0, 255]
BUG: slot 1's attribute read the buffer bound at slot 0
```

Expected: green (`[0, 255, 0, 255]`) in both draws. Seen on Metal (macOS 27.2, Apple M3 Max); the compaction is in
wgpu-core, so every backend is expected to be affected (not verified on others).

## Versions

- Affected: wgpu 29.0.4 (wgpu-core 29.0.4).
- Fixed: wgpu 30.0.0 and later (#9351); verified with 30.0.2.

## Workaround

Do not pass attribute-less layouts before used ones on 29.x: move the used layouts down and remap the slots at
`set_vertex_buffer`, or give the hole a dummy attribute the shader does not read (needs a free shader location).
nsh-webgpu does neither yet (docs/parity.md, commands).
