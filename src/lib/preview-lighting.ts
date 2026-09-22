import * as THREE from 'three'

/** Shared screen buffers and world-space shadow metrics for preview material clones. */
export function createPreviewLighting() {
  return {
    previewAO: { value: null as THREE.Texture | null },
    previewAOIntensity: { value: .65 },
    previewGI: { value: null as THREE.Texture | null },
    previewGIIntensity: { value: 0 },
    previewResolution: { value: new THREE.Vector2(1, 1) },
    previewShadowExtent: { value: new THREE.Vector3(1, 1, 1) },
    previewLightRadius: { value: Math.tan(THREE.MathUtils.degToRad(6)) },
  }
}
export type PreviewLighting = ReturnType<typeof createPreviewLighting>

export function penumbraRadius(distance: number, angularDiameter: number) {
  return Math.max(0, distance) * Math.tan(THREE.MathUtils.degToRad(angularDiameter * .5))
}

const pcss = `
vec2 previewDisk(int i, int count) {
  float angle = float(i) * 2.39996323;
  return vec2(cos(angle), sin(angle)) * sqrt((float(i) + .5) / float(count));
}
float previewDepth(sampler2D map, vec2 uv) {
  if (any(lessThan(uv, vec2(0.))) || any(greaterThan(uv, vec2(1.)))) return 1.;
  return textureLod(map, uv, 0.).r;
}
float getShadow(sampler2D shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 coord) {
  vec3 p = coord.xyz / coord.w;
  vec3 dx = dFdx(p), dy = dFdy(p);
  float det = dx.x * dy.y - dx.y * dy.x;
  vec2 slope = abs(det) > 1e-10 ? vec2(dy.y * dx.z - dx.y * dy.z, dx.x * dy.z - dy.x * dx.z) / det : vec2(0.);
  p.z += shadowBias;
  if (any(lessThan(p, vec3(0.))) || any(greaterThan(p, vec3(1.)))) return 1.;
  vec2 texel = 1. / shadowMapSize;
  vec2 searchRadius = max(texel, p.z * previewShadowExtent.z * previewLightRadius / previewShadowExtent.xy);
  float blockers = 0., blockerDepth = 0.;
  for (int i = 0; i < 16; i++) {
    vec2 offset = previewDisk(i, 16) * searchRadius;
    float depth = previewDepth(shadowMap, p.xy + offset);
    float corrected = depth - dot(slope, offset);
    if (corrected < p.z - .00002) { blockerDepth += corrected; blockers += 1.; }
  }
  if (blockers == 0.) return 1.;
  float separation = max(0., p.z - blockerDepth / blockers) * previewShadowExtent.z;
  vec2 radius = max(texel * .75, separation * previewLightRadius / previewShadowExtent.xy);
  float visibility = 0.;
  for (int i = 0; i < 32; i++) {
    vec2 offset = previewDisk(i, 32) * radius;
    float receiver = p.z + dot(slope, offset);
    visibility += step(receiver, previewDepth(shadowMap, p.xy + offset));
  }
  return mix(1., visibility / 32., shadowIntensity);
}
`

export function applyPreviewLighting(material: THREE.MeshStandardMaterial, lighting: PreviewLighting) {
  material.onBeforeCompile = shader => {
    Object.assign(shader.uniforms, lighting)
    const shadowChunk = THREE.ShaderChunk.shadowmap_pars_fragment.replace(
      /(#else[^\n]*\n\s*)float getShadow\([\s\S]*?(\n\s*#endif\s*\n\s*#if NUM_SUN_LIGHT_SHADOWS)/,
      `$1\n${pcss}\n$2`,
    )
    shader.fragmentShader = `uniform sampler2D previewAO;
uniform sampler2D previewGI;
uniform float previewAOIntensity;
uniform float previewGIIntensity;
uniform vec2 previewResolution;
uniform vec3 previewShadowExtent;
uniform float previewLightRadius;
` + shader.fragmentShader.replace('#include <shadowmap_pars_fragment>', shadowChunk)
    // Transparent surfaces have no matching screen-space depth; keep their IBL intact.
    if (!material.transparent && !(material instanceof THREE.MeshPhysicalMaterial && material.transmission > 0)) {
      shader.fragmentShader = shader.fragmentShader.replace('#include <aomap_fragment>', `#include <aomap_fragment>
vec2 previewUv = gl_FragCoord.xy / previewResolution;
float previewOcclusion = mix(1., texture2D(previewAO, previewUv).r, previewAOIntensity);
reflectedLight.indirectDiffuse *= previewOcclusion;
reflectedLight.indirectSpecular *= computeSpecularOcclusion(saturate(dot(geometryNormal, geometryViewDir)), previewOcclusion, material.roughness);
if (previewGIIntensity > 0.) reflectedLight.indirectDiffuse += material.diffuseColor * texture2D(previewGI, previewUv).rgb * previewGIIntensity;
`)
    }
  }
  material.customProgramCacheKey = () => `preview-lighting-v1-${material.transparent}-${material instanceof THREE.MeshPhysicalMaterial && material.transmission > 0}`
}
