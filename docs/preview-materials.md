# Preview rendering and materials

The viewport uses a multisampled half-float render target, a beauty pass, GTAO with
Poisson denoising, and an output pass for tone mapping and sRGB conversion. AO is
enabled by default in all three modes and disabled for wireframes. Its 0.45 metre
radius stays local when previewing a long consist. The normal/depth pass preserves
base-color alpha cutouts and excludes blended surfaces, lines and sprites. Camera
switches, viewport resizing and thumbnails use the same pipeline.

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

The **Materials** panel edits these overrides. View the results in **Material
Preview** or **Minecraft environment**; Studio deliberately uses neutral gray
shading. Reset material removes the layer's overrides and restores its imported
properties and images.

Existing assets remain readable. Properties discarded by an older import cannot
be recovered from its simplified model document: reimport the original model or
set layer overrides. Advanced PBR properties are saved for preview; current game
exporters still use their existing base-color texture pipeline, with opacity
overrides applied to exported colors/textures. They do not emit shader-pack PBR maps.

Regression coverage includes legacy defaults, property validation, channel packing,
image embedding, material independence, project reopen, cutout AO, texture filtering
and color/data texture separation.
