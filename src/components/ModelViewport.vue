<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { getModelPreview } from '../lib/projects'

const props = withDefaults(defineProps<{ assets: { id: string; visible: boolean }[]; selectedPart?: string; showGrid?: boolean; wireframe?: boolean }>(), { showGrid: true, wireframe: false })
const emit = defineEmits<{ select: [partId: string] }>()
const host = ref<HTMLDivElement>()
let renderer: THREE.WebGLRenderer | undefined
let scene: THREE.Scene | undefined
let camera: THREE.PerspectiveCamera | undefined
let controls: OrbitControls | undefined
let resize: ResizeObserver | undefined
let frame = 0
let content: THREE.Group | undefined
let selection: THREE.BoxHelper | undefined
let grid: THREE.GridHelper | undefined
const loaded = new Map<string, THREE.Object3D>()

async function rebuild() {
  if (!scene) return
  if (content) scene.remove(content)
  loaded.clear(); content = new THREE.Group(); scene.add(content)
  for (const asset of props.assets) {
    try {
      const bytes = await getModelPreview(asset.id)
      const gltf = await new GLTFLoader().parseAsync(bytes, '')
      gltf.scene.visible = asset.visible; gltf.scene.userData.assetId = asset.id
      gltf.scene.traverse(object => { object.userData.assetId = asset.id; if (object instanceof THREE.Mesh) { const materials = Array.isArray(object.material) ? object.material : [object.material]; materials.forEach(material => { material.wireframe = props.wireframe }) } })
      loaded.set(asset.id, gltf.scene); content.add(gltf.scene)
    } catch { /* The editor displays import errors separately. */ }
  }
  frameContent(); updateSelection()
}

function frameContent() {
  if (!content || !camera || !controls) return
  const box = new THREE.Box3().setFromObject(content); if (box.isEmpty()) return
  const center = box.getCenter(new THREE.Vector3()); const size = box.getSize(new THREE.Vector3()).length()
  controls.target.copy(center); camera.position.copy(center).add(new THREE.Vector3(size * .65, size * .4, size * .75)); camera.near = Math.max(.01, size / 1000); camera.far = Math.max(100, size * 20); camera.updateProjectionMatrix(); controls.update()
}

function updateWireframe() {
  content?.traverse(object => { if (object instanceof THREE.Mesh) { const materials = Array.isArray(object.material) ? object.material : [object.material]; materials.forEach(material => { material.wireframe = props.wireframe; material.needsUpdate = true }) } })
}

defineExpose({ fitView: frameContent })

function updateSelection() {
  if (!scene) return
  if (selection) scene.remove(selection)
  selection = undefined
  if (!props.selectedPart) return
  let selected: THREE.Object3D | undefined
  content?.traverse(object => { if (object.userData.partId === props.selectedPart) selected = object })
  if (selected) { selection = new THREE.BoxHelper(selected, 0xc6e2ff); scene.add(selection) }
}

function click(event: MouseEvent) {
  if (!renderer || !camera || !content) return
  const bounds = renderer.domElement.getBoundingClientRect(); const pointer = new THREE.Vector2((event.clientX - bounds.left) / bounds.width * 2 - 1, -(event.clientY - bounds.top) / bounds.height * 2 + 1)
  const raycaster = new THREE.Raycaster(); raycaster.setFromCamera(pointer, camera)
  const hit = raycaster.intersectObject(content, true)[0]?.object
  let current: THREE.Object3D | null | undefined = hit
  while (current && !current.userData.partId) current = current.parent
  if (current?.userData.partId) emit('select', current.userData.partId)
}

onMounted(() => {
  if (!host.value) return
  scene = new THREE.Scene(); scene.background = new THREE.Color(0x151a1f)
  camera = new THREE.PerspectiveCamera(45, 1, .01, 10000)
  renderer = new THREE.WebGLRenderer({ antialias: true }); renderer.setPixelRatio(Math.min(devicePixelRatio, 2)); renderer.outputColorSpace = THREE.SRGBColorSpace; host.value.append(renderer.domElement)
  controls = new OrbitControls(camera, renderer.domElement); controls.enableDamping = true
  scene.add(new THREE.HemisphereLight(0xdcecff, 0x30353a, 2.4)); const sun = new THREE.DirectionalLight(0xffffff, 2.2); sun.position.set(8, 12, 9); scene.add(sun)
  grid = new THREE.GridHelper(100, 100, 0x52606c, 0x28323a); grid.visible = props.showGrid; scene.add(grid); scene.add(new THREE.AxesHelper(2))
  renderer.domElement.addEventListener('click', click)
  resize = new ResizeObserver(() => { if (!host.value || !renderer || !camera) return; const { clientWidth, clientHeight } = host.value; renderer.setSize(clientWidth, clientHeight, false); camera.aspect = clientWidth / Math.max(clientHeight, 1); camera.updateProjectionMatrix() }); resize.observe(host.value)
  const animate = () => { frame = requestAnimationFrame(animate); controls?.update(); selection?.update(); if (scene && camera) renderer?.render(scene, camera) }; animate(); void rebuild()
})
watch(() => props.assets.map(asset => `${asset.id}:${asset.visible}`).join('|'), rebuild)
watch(() => props.selectedPart, updateSelection)
watch(() => props.showGrid, value => { if (grid) grid.visible = value })
watch(() => props.wireframe, updateWireframe)
onBeforeUnmount(() => { cancelAnimationFrame(frame); resize?.disconnect(); if (renderer) { renderer.domElement.removeEventListener('click', click); renderer.dispose(); renderer.domElement.remove() } })
</script>

<template><div ref="host" class="model-viewport"><div v-if="!assets.length" class="viewport-empty">Import a model to start the 3D preview.</div></div></template>

<style scoped>
.model-viewport{position:relative;width:100%;height:100%;min-height:340px;overflow:hidden;background:#151a1f}.model-viewport :deep(canvas){display:block;width:100%;height:100%}.viewport-empty{position:absolute;z-index:1;inset:0;display:grid;place-items:center;color:#77838e;font-size:12px;pointer-events:none}
</style>
