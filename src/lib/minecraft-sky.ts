import * as THREE from 'three'
import { FullScreenQuad } from 'three/examples/jsm/postprocessing/Pass.js'
import type { ViewportSettings } from './viewport-settings.ts'

/** Artistic atmosphere approximation in linear HDR, shared by background, IBL and fog. */
export function minecraftAtmosphere(settings: ViewportSettings) {
  const daylight = THREE.MathUtils.smoothstep(settings.lightElevation, 5, 45)
  return {
    zenith: new THREE.Color().setRGB(.065 + .035 * daylight, .19 + .06 * daylight, .48 + .16 * daylight),
    horizon: new THREE.Color().setRGB(.88 - .38 * daylight, .36 + .29 * daylight, .17 + .65 * daylight),
    sun: new THREE.Color().setRGB(1, .57 + .36 * daylight, .28 + .56 * daylight),
    daylight,
  }
}

const fragmentShader = `
varying vec2 vUv;
uniform vec3 sunDirection, zenithColor, horizonColor, sunColor;
uniform float cloudCover, haze, sunRadius;
void main() {
  float phi=(vUv.x-.5)*6.2831853, latitude=(vUv.y-.5)*3.14159265;
  vec3 d=vec3(cos(phi)*cos(latitude),sin(latitude),sin(phi)*cos(latitude));
  float up=max(0.,d.y), mu=clamp(dot(d,sunDirection),-1.,1.);
  float horizon=exp(-up*(4.+(1.-haze)*3.));
  vec3 sky=mix(zenithColor,horizonColor,horizon*(.65+haze*.3));
  // Broad Rayleigh-like angular variation and a narrow forward-scattering aureole.
  sky*=.82+.18*mu*mu;
  float forward=pow(max(0.,mu),mix(90.,18.,haze));
  sky+=sunColor*forward*(.25+haze*.55)*(1.-.4*up);
  float angle=acos(mu);
  float disk=1.-smoothstep(sunRadius*.85,sunRadius*1.15,angle);
  sky+=sunColor*(disk*12.+.4*exp(-angle/max(.009,sunRadius*3.)));
  // Cloud cover gently shifts the ambient balance; voxel clouds are scene geometry.
  sky*=1.-cloudCover*.12;
  vec3 ground=vec3(.10,.125,.075)*(1.+sunDirection.y*.4);
  sky=mix(ground,sky,smoothstep(-.12,.015,d.y));
  gl_FragColor=vec4(sky,1.);
}`

/** Owns a cached equirectangular sky. Steady-state rendering only samples the texture. */
export class MinecraftSky {
  private target = new THREE.WebGLRenderTarget(1024,512,{type:THREE.HalfFloatType,depthBuffer:false})
  private material = new THREE.ShaderMaterial({
    uniforms:{sunDirection:{value:new THREE.Vector3()},zenithColor:{value:new THREE.Color()},horizonColor:{value:new THREE.Color()},sunColor:{value:new THREE.Color()},cloudCover:{value:.5},haze:{value:.35},sunRadius:{value:.02}},
    vertexShader:'varying vec2 vUv; void main(){vUv=uv;gl_Position=vec4(position.xy,0.,1.);}',fragmentShader,depthTest:false,depthWrite:false,blending:THREE.NoBlending,
  })
  private quad = new FullScreenQuad(this.material)
  constructor() {this.target.texture.mapping=THREE.EquirectangularReflectionMapping;this.target.texture.colorSpace=THREE.LinearSRGBColorSpace}
  bake(renderer:THREE.WebGLRenderer,settings:ViewportSettings,direction:THREE.Vector3) {
    const atmosphere=minecraftAtmosphere(settings),u=this.material.uniforms
    u.sunDirection!.value.copy(direction);u.zenithColor!.value.copy(atmosphere.zenith);u.horizonColor!.value.copy(atmosphere.horizon);u.sunColor!.value.copy(atmosphere.sun)
    u.cloudCover!.value=settings.cloudCover;u.haze!.value=settings.skyHaze;u.sunRadius!.value=THREE.MathUtils.degToRad(settings.lightSize*.5)
    const target=renderer.getRenderTarget(),xr=renderer.xr.enabled,autoClear=renderer.autoClear
    try {renderer.xr.enabled=false;renderer.autoClear=true;renderer.setRenderTarget(this.target);this.quad.render(renderer)}
    finally {renderer.setRenderTarget(target);renderer.xr.enabled=xr;renderer.autoClear=autoClear}
    return this.target.texture
  }
  get texture(){return this.target.texture}
  dispose(){this.target.dispose();this.material.dispose();this.quad.dispose()}
}
