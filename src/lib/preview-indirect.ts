import * as THREE from 'three'
import { Pass, FullScreenQuad } from 'three/examples/jsm/postprocessing/Pass.js'

const vertexShader = `varying vec2 vUv;
void main() { vUv = uv; gl_Position = vec4(position.xy, 0., 1.); }`
const geometry = `
varying vec2 vUv;
uniform sampler2D tDepth;
uniform sampler2D tNormal;
uniform mat4 inverseProjection;
uniform mat4 projection;
vec3 positionAt(vec2 uv) {
  vec4 p = inverseProjection * vec4(uv * 2. - 1., textureLod(tDepth, uv, 0.).r * 2. - 1., 1.);
  return p.xyz / p.w;
}
vec3 normalAt(vec2 uv) { return normalize(textureLod(tNormal, uv, 0.).xyz * 2. - 1.); }
`
const traceShader = geometry + `
uniform sampler2D tRadiance;
uniform vec2 resolution;
void main() {
  if (textureLod(tDepth, vUv, 0.).r >= .999999) { gl_FragColor = vec4(0.); return; }
  vec3 p = positionAt(vUv), n = normalAt(vUv);
  vec3 axis = abs(n.z) < .9 ? vec3(0.,0.,1.) : vec3(0.,1.,0.);
  vec3 tangent = normalize(cross(axis,n)), bitangent = cross(n,tangent);
  // Fixed screen-space sequence; no previous-frame radiance or temporal ghosting.
  float rotation = fract(dot(floor(vUv * resolution),vec2(.06711056,.00583715)) * 52.9829189) * 6.2831853;
  vec3 incoming = vec3(0.);
  for (int ray = 0; ray < 8; ray++) {
    float r = sqrt((float(ray)+.5)/8.), angle = float(ray)*2.39996323+rotation;
    vec3 direction = tangent*(cos(angle)*r)+bitangent*(sin(angle)*r)+n*sqrt(1.-r*r);
    for (int stepIndex = 1; stepIndex <= 12; stepIndex++) {
      float distance = .025 + 2.5 * pow(float(stepIndex)/12., 2.);
      vec3 q = p + n*.025 + direction*distance;
      vec4 clip = projection * vec4(q,1.);
      if (clip.w <= 0.) break;
      vec2 uv = clip.xy/clip.w*.5+.5;
      if (any(lessThan(uv,vec2(.002))) || any(greaterThan(uv,vec2(.998)))) break;
      if (textureLod(tDepth, uv, 0.).r >= .999999) continue;
      vec3 hit = positionAt(uv);
      float delta = hit.z-q.z;
      // Thickness is in model metres; reject crossings behind unrelated surfaces.
      if (delta > 0. && delta < .12 + distance*.06) {
        if (dot(normalAt(uv),-direction) > .05 && length(hit-p) > .045) {
          vec3 radiance = max(vec3(0.),textureLod(tRadiance, uv, 0.).rgb);
          float peak = max(radiance.r,max(radiance.g,radiance.b));
          radiance *= min(1.,3./max(peak,.001));
          vec2 edge = min(uv,1.-uv);
          float confidence = smoothstep(0.,.06,min(edge.x,edge.y));
          incoming += radiance * confidence * (1.-smoothstep(1.5,2.5,distance));
        }
        break;
      }
    }
  }
  // Cosine-weighted hemisphere estimates irradiance / pi. Misses use the scene IBL.
  gl_FragColor = vec4(incoming/8.,1.);
}`
const denoiseShader = geometry + `
uniform sampler2D tIndirect;
uniform vec2 resolution;
void main() {
  if (textureLod(tDepth, vUv, 0.).r >= .999999) { gl_FragColor=vec4(0.); return; }
  vec3 p=positionAt(vUv),n=normalAt(vUv),sum=vec3(0.); float weightSum=0.;
  for (int y=-2;y<=2;y++) for (int x=-2;x<=2;x++) {
    vec2 uv=clamp((floor(vUv*resolution)+vec2(float(x),float(y))+.5)/resolution,.5/resolution,1.-.5/resolution);
    vec3 delta=positionAt(uv)-p;
    float weight=exp(-float(x*x+y*y)/5.)*pow(max(0.,dot(n,normalAt(uv))),16.);
    weight*=exp(-abs(dot(delta,n))*50.-length(delta)*2.);
    if(textureLod(tDepth, uv, 0.).r>=.999999)weight=0.;
    sum+=textureLod(tIndirect, uv, 0.).rgb*weight;weightSum+=weight;
  }
  gl_FragColor=vec4(sum/max(weightSum,.00001),1.);
}`

/** One local diffuse bounce from visible surfaces, at half resolution with bilateral upsampling. */
export class PreviewIndirectPass extends Pass {
  camera: THREE.Camera
  private trace: THREE.ShaderMaterial
  private denoise: THREE.ShaderMaterial
  private quad = new FullScreenQuad()
  private raw = new THREE.WebGLRenderTarget(1,1,{type:THREE.HalfFloatType,depthBuffer:false,minFilter:THREE.NearestFilter,magFilter:THREE.NearestFilter})
  private filtered = new THREE.WebGLRenderTarget(1,1,{type:THREE.HalfFloatType,depthBuffer:false})
  constructor(camera: THREE.Camera, depth: THREE.Texture, normal: THREE.Texture) {
    super(); this.camera=camera;this.needsSwap=false
    const uniforms = () => ({tDepth:{value:depth},tNormal:{value:normal},inverseProjection:{value:new THREE.Matrix4()},projection:{value:new THREE.Matrix4()},resolution:{value:new THREE.Vector2(1,1)}})
    this.trace=new THREE.ShaderMaterial({uniforms:{...uniforms(),tRadiance:{value:null}},vertexShader,fragmentShader:traceShader,depthTest:false,depthWrite:false,blending:THREE.NoBlending})
    this.denoise=new THREE.ShaderMaterial({uniforms:{...uniforms(),tIndirect:{value:this.raw.texture}},vertexShader,fragmentShader:denoiseShader,depthTest:false,depthWrite:false,blending:THREE.NoBlending})
  }
  get texture() { return this.filtered.texture }
  setSize(width:number,height:number) {
    const w=Math.max(1,Math.ceil(width/2)),h=Math.max(1,Math.ceil(height/2))
    this.raw.setSize(w,h);this.filtered.setSize(width,height)
    this.trace.uniforms.resolution!.value.set(w,h);this.denoise.uniforms.resolution!.value.set(w,h)
  }
  render(renderer:THREE.WebGLRenderer,_write:THREE.WebGLRenderTarget,read:THREE.WebGLRenderTarget) {
    const camera=this.camera as THREE.PerspectiveCamera | THREE.OrthographicCamera
    for(const material of [this.trace,this.denoise]) {
      material.uniforms.inverseProjection!.value.copy(camera.projectionMatrixInverse)
      material.uniforms.projection!.value.copy(camera.projectionMatrix)
    }
    this.trace.uniforms.tRadiance!.value=read.texture
    this.quad.material=this.trace;renderer.setRenderTarget(this.raw);this.quad.render(renderer)
    this.quad.material=this.denoise;renderer.setRenderTarget(this.filtered);this.quad.render(renderer)
  }
  dispose() {this.raw.dispose();this.filtered.dispose();this.trace.dispose();this.denoise.dispose();this.quad.dispose()}
}
