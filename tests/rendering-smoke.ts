import * as THREE from 'three'
import { PreviewRenderer } from '../src/lib/preview-renderer'
import { PreviewEnvironment } from '../src/lib/preview-environment'
import { PreviewEnvironmentMap } from '../src/lib/preview-environment-map'
import { defaultViewportSettings } from '../src/lib/viewport-settings'

// Open /tests/rendering.html with pnpm dev. No application/project data is touched.
const renderer = new THREE.WebGLRenderer({antialias:true,preserveDrawingBuffer:true})
renderer.setSize(720,480);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping
renderer.shadowMap.enabled=true;renderer.shadowMap.type=THREE.BasicShadowMap;renderer.shadowMap.autoUpdate=false
document.body.append(renderer.domElement)
const scene=new THREE.Scene(),root=new THREE.Group();scene.add(root)
const camera=new THREE.PerspectiveCamera(45,1.5,.05,100);camera.position.set(6,5,7);camera.lookAt(0,1,0);camera.updateMatrixWorld()
const pipeline=new PreviewRenderer(renderer,scene,camera);pipeline.setSize(720,480,1)
const map=new PreviewEnvironmentMap(renderer),environment=new PreviewEnvironment(scene,undefined,pipeline.lighting)
const pixels=new Uint8Array([255,255,255,255, 20,80,220,255, 20,80,220,255, 255,255,255,255])
const checker=new THREE.DataTexture(pixels,2,2);checker.colorSpace=THREE.SRGBColorSpace;checker.magFilter=THREE.NearestFilter;checker.needsUpdate=true
const cube=new THREE.Mesh(new THREE.BoxGeometry(1.5,1.5,1.5),new THREE.MeshStandardMaterial({map:checker,roughness:.6}));cube.position.set(.5,1.5,0);root.add(cube)
const wall=new THREE.Mesh(new THREE.BoxGeometry(.15,2,3),new THREE.MeshStandardMaterial({color:0xff0808,roughness:1}));wall.position.set(-1.2,1,0);root.add(wall)
const sphere=new THREE.Mesh(new THREE.SphereGeometry(.55,40,32),new THREE.MeshStandardMaterial({color:0xffffff,metalness:.9,roughness:.22}));sphere.position.set(1.7,.55,-1.4);root.add(sphere)
let settings={...defaultViewportSettings('material'),lightAzimuth:300,lightElevation:55},mode='material',gi=true
const errors:string[]=[];renderer.debug.onShaderError=(_gl,_program,_vertex,_fragment)=>{errors.push('Shader compilation failed')}
function render(){pipeline.render()}
function apply(){pipeline.clearMaterials();environment.setEnvironmentMap(map.update(mode as 'material',settings));environment.apply(root,mode as 'material',settings);pipeline.setAO(settings.ambientOcclusion);pipeline.setIndirect(gi?settings.indirectIntensity:0);renderer.shadowMap.needsUpdate=true;render()}
function capture(){render();const data=new Uint8Array(720*480*4);const gl=renderer.getContext();gl.readPixels(0,0,720,480,gl.RGBA,gl.UNSIGNED_BYTE,data);return data}
function difference(a:Uint8Array,b:Uint8Array){let changed=0,total=0;for(let i=0;i<a.length;i+=4){let delta=0;for(let c=0;c<3;c++)delta+=Math.abs(a[i+c]!-b[i+c]!);if(delta>3)changed++;total+=delta}return {changed,mean:total/(a.length/4*3)}}
function run(){
  const results:any[]=[];mode='material';settings={...defaultViewportSettings('material'),lightAzimuth:300,lightElevation:55};gi=false
  settings.lightSize=.1;apply();const hard=capture();settings.lightSize=18;apply();const soft=capture();results.push({check:'PCSS responds to source size',...difference(hard,soft)})
  gi=true;apply();const bounce=capture();results.push({check:'Visible local bounce',...difference(soft,bounce)})
  gi=false;settings.ambientOcclusion=false;apply();results.push({check:'AO darkens occluded environment',...difference(soft,capture())})
  gi=false;settings.environmentIntensity=0;settings.ambientOcclusion=false;apply();const direct=capture();settings.ambientOcclusion=true;apply();results.push({check:'AO leaves direct lighting unchanged',...difference(direct,capture())})
  const ortho=new THREE.OrthographicCamera(-5,5,3.33,-3.33,.05,100);ortho.position.copy(camera.position);ortho.lookAt(0,1,0);ortho.updateMatrixWorld();pipeline.setCamera(ortho);gi=true;apply();results.push({check:'Orthographic rendering',nonzero:capture().some(v=>v>0)})
  pipeline.setSize(240,160,1);renderer.setSize(240,160);render();pipeline.setSize(720,480,1);renderer.setSize(720,480);pipeline.setCamera(camera)
  settings.environmentIntensity=.8;mode='studio';apply();results.push({check:'Studio keeps texture',preserved:(cube.material as THREE.MeshStandardMaterial).map===checker})
  const glError=renderer.getContext().getError();results.push({check:'GPU errors',errors,glError})
  const passed=results[0].changed>100 && results[1].changed>100 && results[2].changed>100 && results[3].mean<.001 && results[4].nonzero && results[5].preserved && !errors.length && glError===0
  document.querySelector('#results')!.textContent=JSON.stringify({passed,results},null,2)
}
document.querySelector('#run')!.addEventListener('click',run)
document.querySelector('#soft')!.addEventListener('click',()=>{mode='material';settings.lightSize=18;apply()})
document.querySelector('#hard')!.addEventListener('click',()=>{mode='material';settings.lightSize=.1;apply()})
document.querySelector('#gi')!.addEventListener('click',()=>{gi=!gi;apply();document.querySelector('#results')!.textContent=`Local bounce: ${gi}`})
document.querySelector('#studio')!.addEventListener('click',()=>{mode='studio';apply()})
apply();document.querySelector('#results')!.textContent='Ready. The checker texture must remain visible in Studio.'
window.addEventListener('beforeunload',()=>{pipeline.dispose();environment.dispose();map.dispose();root.traverse(o=>{if(o instanceof THREE.Mesh){o.geometry.dispose();(o.material as THREE.Material).dispose()}});checker.dispose();renderer.dispose()})
