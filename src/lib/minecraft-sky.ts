import * as THREE from 'three'
import { FullScreenQuad } from 'three/examples/jsm/postprocessing/Pass.js'
import type { ViewportSettings } from './viewport-settings.ts'

/** Artistic atmosphere approximation in linear HDR, shared by background, IBL and fog. */
export function minecraftAtmosphere(settings: ViewportSettings) {
  const altitude = Math.sin((settings.timeOfDay - 6) * Math.PI / 12)
  const daylight = THREE.MathUtils.smoothstep(altitude, -.08, .25)
  const twilight = (1-THREE.MathUtils.smoothstep(Math.abs(altitude), .02, .28))*THREE.MathUtils.smoothstep(altitude, -.2, -.02)
  const night = 1-daylight
  const zenith = new THREE.Color().setRGB(.006+.094*daylight,.012+.238*daylight,.035+.605*daylight)
  const horizon = new THREE.Color().setRGB(.012+.49*daylight+.60*twilight,.02+.63*daylight+.17*twilight,.045+.77*daylight+.05*twilight)
  return {
    zenith,horizon,
    sun: new THREE.Color().setRGB(1-.58*night,.93-.44*night-.18*twilight,.84-.08*night-.42*twilight),
    daylight,sunStrength:THREE.MathUtils.smoothstep(altitude,-.08,.05),
    directFactor:.03+.97*daylight,environmentFactor:.06+.94*daylight,
  }
}

const fragmentShader = `
varying vec2 vUv;
uniform vec3 sunDirection, zenithColor, horizonColor, sunColor;
uniform float cloudCover, haze, sunRadius, sunStrength;
void main() {
  float phi=(vUv.x-.5)*6.2831853, latitude=(vUv.y-.5)*3.14159265;
  vec3 d=vec3(cos(phi)*cos(latitude),sin(latitude),sin(phi)*cos(latitude));
  float up=max(0.,d.y), mu=clamp(dot(d,sunDirection),-1.,1.);
  float horizon=exp(-up*(4.+(1.-haze)*3.));
  vec3 sky=mix(zenithColor,horizonColor,horizon*(.65+haze*.3));
  // Broad Rayleigh-like angular variation and a narrow forward-scattering aureole.
  sky*=.82+.18*mu*mu;
  float forward=pow(max(0.,mu),mix(90.,18.,haze));
  sky+=sunColor*forward*(.25+haze*.55)*(1.-.4*up)*sunStrength;
  float angle=acos(mu);
  float disk=1.-smoothstep(sunRadius*.85,sunRadius*1.15,angle);
  sky+=sunColor*(disk*12.+.4*exp(-angle/max(.009,sunRadius*3.)))*sunStrength;
  // Cloud cover gently shifts the ambient balance; voxel clouds are scene geometry.
  sky*=1.-cloudCover*.12;
  vec3 ground=zenithColor*.35;
  sky=mix(ground,sky,smoothstep(-.12,.015,d.y));
  gl_FragColor=vec4(sky,1.);
}`

/** Owns a cached equirectangular sky. Steady-state rendering only samples the texture. */
export class MinecraftSky {
  private target = new THREE.WebGLRenderTarget(1024,512,{type:THREE.HalfFloatType,depthBuffer:false})
  private material = new THREE.ShaderMaterial({
    uniforms:{sunDirection:{value:new THREE.Vector3()},zenithColor:{value:new THREE.Color()},horizonColor:{value:new THREE.Color()},sunColor:{value:new THREE.Color()},cloudCover:{value:.5},haze:{value:.35},sunRadius:{value:.02},sunStrength:{value:1}},
    vertexShader:'varying vec2 vUv; void main(){vUv=uv;gl_Position=vec4(position.xy,0.,1.);}',fragmentShader,depthTest:false,depthWrite:false,blending:THREE.NoBlending,
  })
  private quad = new FullScreenQuad(this.material)
  constructor() {this.target.texture.mapping=THREE.EquirectangularReflectionMapping;this.target.texture.colorSpace=THREE.LinearSRGBColorSpace}
  bake(renderer:THREE.WebGLRenderer,settings:ViewportSettings,direction:THREE.Vector3) {
    const atmosphere=minecraftAtmosphere(settings),u=this.material.uniforms
    u.sunDirection!.value.copy(direction);u.zenithColor!.value.copy(atmosphere.zenith);u.horizonColor!.value.copy(atmosphere.horizon);u.sunColor!.value.copy(atmosphere.sun)
    u.sunStrength!.value=atmosphere.sunStrength
    u.cloudCover!.value=settings.cloudCover;u.haze!.value=settings.skyHaze;u.sunRadius!.value=THREE.MathUtils.degToRad(settings.lightSize*.5)
    const target=renderer.getRenderTarget(),xr=renderer.xr.enabled,autoClear=renderer.autoClear
    try {renderer.xr.enabled=false;renderer.autoClear=true;renderer.setRenderTarget(this.target);this.quad.render(renderer)}
    finally {renderer.setRenderTarget(target);renderer.xr.enabled=xr;renderer.autoClear=autoClear}
    return this.target.texture
  }
  get texture(){return this.target.texture}
  dispose(){this.target.dispose();this.material.dispose();this.quad.dispose()}
}
