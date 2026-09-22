# Preview rendering and materials

All three modes preserve the imported textures and PBR channels. Studio uses fixed,
neutral world-space lighting; Material adds an optional floor; Minecraft uses a
procedural sky environment and grass scenery. The PMREM environment's bright region
tracks the main light direction. Environment and direct-light intensities remain
independent art controls. This is raster rendering, with no path tracing.

The half-float pipeline renders normal/depth, GTAO, direct plus environment lighting,
one local screen-space diffuse bounce, then the final material pass, SMAA and output
conversion. Up to 4x MSAA handles geometric edges; SMAA runs before tone mapping and
sRGB conversion. AO affects indirect diffuse/specular lighting only, leaving direct
illumination and emission intact. Its 0.45 metre radius stays local for long consists.
Alpha cutouts participate in the normal pass; blended surfaces, lines and sprites
are excluded. Wireframes disable AO and local bounce. Camera switches, resizing and
thumbnails use the same pipeline.

Directional soft shadows use PCSS: search blockers, estimate penumbra from their
world-space distance to the receiver and the light's angular diameter, then filter.
Contact stays sharper and distant shadows soften. **Light angular size** controls
softness, independently of shadow-map resolution. Studio/Material default to 12
degrees; Minecraft defaults to an artistic 2 degrees. Increasing shadow quality
raises the shadow-map resolution from 2048 to 4096.

**Local indirect light** controls an approximate single diffuse bounce from visible
surfaces within 2.5 metres. Eight cosine-weighted screen-space rays use twelve depth
steps each at half resolution, followed by depth/normal-aware filtering and
upsampling. It uses the current frame only: there is no accumulating feedback or
history ghosting. Screen-space misses, hidden surfaces and off-screen emitters rely
on the environment lighting. Small/thin geometry can still lose bounce detail;
specular radiance is clamped to limit fireflies. This is not full global illumination.
Set local indirect light to zero to skip its extra scene pass and screen-space work.

Model textures default to linear magnification, trilinear mipmapping and up to 8x
anisotropy, limited by the GPU. The **Pixelated textures** option restores nearest
magnification while retaining mipmaps for stable distant rendering. Color and
emission maps use sRGB; normal, occlusion and metallic/roughness maps use data
color space, including when the same image is used for multiple channels.

## Material data flow

`ModelMaterial.properties` stores imported source properties in the model document.
`MaterialBinding.properties` stores optional per-layer overrides in the train
document. Missing properties retain legacy defaults. Editing a layer never changes
the shared source asset. Both fields persist in `.mtrpack` projects.

Supported properties: metalness, roughness, emission color, opacity, opaque/cutout/
blended alpha mode, alpha cutoff, normal strength and double-sided rendering.
Supported additional texture channels: normal, metalness, roughness, emission and
occlusion. The existing base-color texture binding remains compatible.

Source texture references resolve through imported dependencies. Override texture
references are stored image blob hashes. Previews embed normalized PNG images in
GLB. Separate scalar maps are sampled from their red channel and packed into glTF's
green roughness and blue metalness channels without applying a color transfer
function. A missing scalar map contributes 1 so the scalar factor remains effective.

## Source format support

- OBJ/MTL: `Pm`, `Pr`, `Ke`, `d`, `norm`, `map_Pm`, `map_Pr`, `map_Ke`, `map_AO`,
  and the existing `Kd`/`map_Kd`. When `Pr` is absent, `Ns` is approximated as
  `sqrt(2 / (Ns + 2))`. Height/bump maps are not interpreted as tangent normals.
- FBX: common ufbx PBR metalness, roughness, opacity, normal, emission and AO
  properties and image references, with legacy normal/emission/shininess fallbacks.
  Arbitrary procedural and layered shader graphs are not evaluated.
- MQO: base color/texture and opacity, with `power` converted to approximate
  roughness and `emi` converted to emission color.

The **Materials** panel edits these overrides. View the results in **Studio**, **Material Preview** or **Minecraft environment**. Reset material removes the layer's overrides and restores its imported
properties and images.

Existing assets remain readable. Properties discarded by an older import cannot
be recovered from its simplified model document: reimport the original model or
set layer overrides. Advanced PBR properties are saved for preview; current game
exporters still use their existing base-color texture pipeline, with opacity
overrides applied to exported colors/textures. They do not emit shader-pack PBR maps.

Regression coverage includes legacy defaults, property validation, channel packing,
image embedding, material independence, project reopen, cutout AO, texture filtering
and color/data texture separation.

For a GPU regression check, run `pnpm dev` and open `/tests/rendering.html`.
**Run GPU checks** compares soft/hard source sizes, local bounce and AO toggles,
checks that AO leaves direct-only lighting unchanged within 8-bit rounding,
and exercises orthographic cameras, thumbnail sizes and Studio texture retention.
The fixture is isolated from project data. The result must report `passed: true`
and no shader/WebGL errors. Inspect the soft shadow and checker texture visually.
