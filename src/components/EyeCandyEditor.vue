<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ArrowLeft, Eye, EyeOff, Maximize, Undo2, Redo2, Upload } from '@lucide/vue'
import { t } from '../i18n'
import type { PreviewLayer } from './ModelViewport.vue'
import LoadingProgress from './LoadingProgress.vue'
const ModelViewport = defineAsyncComponent(() => import('./ModelViewport.vue'))
import ModelHierarchy from './ModelHierarchy.vue'
import MaterialEditor from './MaterialEditor.vue'
import ViewportModeControls from './ViewportModeControls.vue'
import { defaultViewportSettings, type PreviewRenderMode } from '../lib/viewport-settings'
import { replacementEyeCandyModel } from '../lib/eye-candy-model'
import { EditorHistory } from '../lib/editor-history'
import { SaveQueue } from '../lib/save-queue'
import { analyzeModelImport, chooseModelDependency, chooseModelFile, chooseTextureFile, getEyeCandy, getModelAsset, importEyeCandyModel, importTextureFile, listAssets, updateEyeCandy, type AssetDefinition, type ContentEntry, type ModelCatalogItem, type MaterialBinding, type MaterialProperties, type ModelLayer, type ModelTransform, type EyeCandyDefinition, type TextureChannel, type TextureCatalogItem, type ValidationIssue } from '../lib/projects'

const props = defineProps<{ projectPath: string; entry: ContentEntry }>()
const emit = defineEmits<{ back: []; export: []; changed: [entry: ContentEntry]; status: [value: 'saving'|'saved'|'failed']; error: [message: string]; ready: [] }>()
const eyeCandy = ref<EyeCandyDefinition>()
const assets = ref<Record<string, AssetDefinition>>({})
const selectedLayerId = ref(''), selectedPartId = ref(''), selectedInstanceKey = ref(''), query = ref('')
const busy = ref(false), loadError = ref('')
const viewport = ref<{fitView:()=>void}>()
const viewportReady=ref(false)
const root = ref<HTMLElement>()
const mode = ref<PreviewRenderMode>('material'), grid = ref(false), wireframe = ref(false)
const settings = ref({ studio: defaultViewportSettings('studio'), material: defaultViewportSettings('material'), minecraft: defaultViewportSettings('minecraft') })
const texturePicker = ref<{materialId:string;channel?:TextureChannel;layerId:string}>()
const textures = ref<TextureCatalogItem[]>([])
const modelPicker = ref(false), models = ref<ModelCatalogItem[]>([])
const saveFailed = ref(false)
let hydrating = false, alive = true
let timer: ReturnType<typeof setTimeout> | undefined
let operation: Promise<void> | undefined
const legacyModels = computed(() => (eyeCandy.value?.models.length || 0) > 1)
const selected = computed(() => eyeCandy.value?.models.find(layer => layer.id === selectedLayerId.value))
const asset = computed(() => selected.value ? assets.value[selected.value.assetId] : undefined)
const previewLayers = computed<PreviewLayer[]>(() => (eyeCandy.value?.models || []).map(layer => ({ key:layer.id, carriageId:eyeCandy.value!.id, layerId:layer.id, assetId:layer.assetId, visible:layer.visible, z:0, reversed:false, bogieOffset:0, flipV:layer.flipTextureV, legacyUvCorrection:assets.value[layer.assetId]?.legacyUvCorrection, bindings:layer.materialBindings, hiddenParts:layer.hiddenParts||[], transform:layer.transform, partTransforms:layer.partTransforms, renderStage:layer.renderStage, partRenderStages:layer.partRenderStages })))
const history = new EditorHistory<EyeCandyDefinition>({normalize: document => ({...document,revision:0})})
const historyTick = ref(0)
const canUndo = computed(() => { void historyTick.value; return !!eyeCandy.value && !legacyModels.value && (history.canUndo || history.hasChanges(eyeCandy.value)) })
const canRedo = computed(() => { void historyTick.value; return !!eyeCandy.value && !legacyModels.value && history.canRedo && !history.hasChanges(eyeCandy.value) })
const identityTransform = (): ModelTransform => ({translation:[0,0,0],rotation:[0,0,0],scale:[1,1,1]})
const targetTransform = computed(() => selectedPartId.value ? selected.value?.partTransforms?.[selectedPartId.value] : selected.value?.transform)
const axes = ['axisX','axisY','axisZ'] as const
function transformValue(field: keyof ModelTransform, index: number) { const value = (targetTransform.value || identityTransform())[field][index]!; return field === 'rotation' ? Math.round(value * 180 / Math.PI * 10000) / 10000 : value }
function changeTransform(field: keyof ModelTransform, index: number, event: Event) {
  const value = (event.target as HTMLInputElement).valueAsNumber
  if (!selected.value || !Number.isFinite(value) || (field === 'scale' && value <= 0)) return
  const layer = selected.value
  if (selectedPartId.value) { layer.partTransforms ||= {}; layer.partTransforms[selectedPartId.value] ||= identityTransform() }
  else layer.transform ||= identityTransform()
  const target = selectedPartId.value ? layer.partTransforms![selectedPartId.value]! : layer.transform!
  target[field][index] = field === 'rotation' ? value * Math.PI / 180 : value
}
function resetTransform() { modify(() => { if (!selected.value) return; if (selectedPartId.value) delete selected.value.partTransforms?.[selectedPartId.value]; else selected.value.transform = identityTransform() }) }
async function travelHistory(direction: 'undo'|'redo') {
  if (!eyeCandy.value || busy.value || legacyModels.value) return
  perform(async () => {
    const restored = history[direction](eyeCandy.value!); historyTick.value++
    if (!restored) return
    restored.revision = eyeCandy.value!.revision
    eyeCandy.value = restored
    await loadAssets(); await flushEdits(false)
  }, false)
}
async function writeSnapshot() {
  if (!eyeCandy.value) return
  const snapshot: EyeCandyDefinition = JSON.parse(JSON.stringify(eyeCandy.value))
  const saved = await updateEyeCandy(props.projectPath, snapshot, snapshot.revision)
  hydrating = true; eyeCandy.value.revision = saved.revision; hydrating = false
  emit('changed', {...props.entry, name:saved.name, updatedAt:Date.now()})
}
let queue = new SaveQueue(writeSnapshot)
function report(cause: unknown) { saveFailed.value=true; emit('status','failed'); emit('error', cause instanceof Error ? cause.message : String(cause)) }
async function load() {
  busy.value=true
  try {
    hydrating=true; eyeCandy.value=await getEyeCandy(props.projectPath,props.entry.id)
    await loadAssets(); selectedLayerId.value=eyeCandy.value.models[0]?.id||''; loadError.value=''
    history.reset(eyeCandy.value);historyTick.value++;queue=new SaveQueue(writeSnapshot);saveFailed.value=false;emit('error','');emit('status','saved'); await nextTick(); emit('ready')
  } catch(cause) { loadError.value=String(cause); report(cause) } finally { hydrating=false;busy.value=false }
}
async function loadAssets() { for (const layer of eyeCandy.value?.models||[]) if (!assets.value[layer.assetId]) assets.value[layer.assetId]=await getModelAsset(layer.assetId) }
watch(eyeCandy, () => { if(hydrating)return; queue.markDirty(); emit('status','saving'); clearTimeout(timer); timer=setTimeout(()=>void flush().catch(()=>{}),800) }, {deep:true,flush:'sync'})
async function flushEdits(record = true) { clearTimeout(timer); try { if(record && eyeCandy.value){history.record(eyeCandy.value);historyTick.value++} await queue.flush(); saveFailed.value=false;emit('status','saved') } catch(cause) { report(cause); throw cause } }
async function flush() { if(operation)await operation; await flushEdits() }
function perform(action: () => Promise<void>, record = true) {
  if(busy.value)return
  busy.value=true
  operation=(async()=>{try{await flushEdits(record);await action()}catch(cause){report(cause);throw cause}finally{busy.value=false;operation=undefined}})()
  void operation.catch(()=>{})
}
function importModel() { perform(async()=>{
  if(!eyeCandy.value)return
  const path=await chooseModelFile(); if(!path)return
  const overrides: Record<string,string>={}
  if(path) {
    let analysis=await analyzeModelImport(path,overrides)
    while(analysis.missingDependencies.length){for(const missing of analysis.missingDependencies){const name=missing.split(/[\\/]/).pop()||missing;const chosen=await chooseModelDependency(name);if(!chosen)return;overrides[name]=chosen}analysis=await analyzeModelImport(path,overrides)}
  }
  emit('status','saving')
  const replacingLegacy=legacyModels.value
  const result=await importEyeCandyModel(eyeCandy.value.id,path,overrides,eyeCandy.value.revision)
  if(!alive)return
  hydrating=true;eyeCandy.value=result.eyeCandy;assets.value[result.asset.id]=result.asset;hydrating=false
  selectedLayerId.value=result.eyeCandy.models[0]?.id||'';selectedPartId.value=''
  if(replacingLegacy)history.reset(eyeCandy.value)
  history.record(eyeCandy.value);historyTick.value++;emit('changed',{...props.entry,name:result.eyeCandy.name,updatedAt:Date.now()});emit('status','saved')
}) }
function modify(action:()=>void) { perform(async()=>{action();await flushEdits()}) }
function visibility(id:string|null,parts?:string[]) { if(legacyModels.value)return;modify(()=>{
  const layers=id?eyeCandy.value!.models.filter(layer=>layer.id===id):eyeCandy.value!.models
  if(parts)for(const layer of layers){const hidden=new Set(layer.hiddenParts||[]),hide=parts.some(part=>!hidden.has(part));for(const part of parts)hide?hidden.add(part):hidden.delete(part);layer.hiddenParts=[...hidden]}
  else {const visible=!layers.some(layer=>layer.visible);for(const layer of layers)layer.visible=visible}
}) }
function select(layerId:string,partId='',instanceKey=''){selectedLayerId.value=layerId;selectedPartId.value=partId;selectedInstanceKey.value=instanceKey}
watch(selectedLayerId,()=>{selectedInstanceKey.value=''},{flush:'sync'})
function binding(layer:ModelLayer,id:string):MaterialBinding {let value=layer.materialBindings.find(item=>item.materialId===id);if(!value){value={materialId:id};layer.materialBindings.push(value)}return value}
function materialChanged(id:string,properties:MaterialProperties){if(selected.value)binding(selected.value,id).properties=properties}
function setTexture(layer:ModelLayer,id:string,hash:string,channel?:TextureChannel){const value=binding(layer,id);if(channel)value.properties={...value.properties,maps:{...value.properties?.maps,[channel]:hash}};else value.textureAssetId=hash}
function importTexture(id:string,channel?:TextureChannel){perform(async()=>{const layer=selected.value;if(!layer)return;const path=await chooseTextureFile();if(!path)return;setTexture(layer,id,await importTextureFile(path),channel);await flushEdits()})}
function pickTexture(id:string,channel?:TextureChannel){perform(async()=>{textures.value=(await listAssets(props.projectPath)).textures;texturePicker.value={materialId:id,channel,layerId:selectedLayerId.value}})}
function applyTexture(hash:string){modify(()=>{const request=texturePicker.value,layer=eyeCandy.value?.models.find(item=>item.id===request?.layerId);if(request&&layer)setTexture(layer,request.materialId,hash,request.channel);texturePicker.value=undefined})}
async function reloadSaved(){if(busy.value||!window.confirm(t('eyeCandyReloadHint')))return;clearTimeout(timer);busy.value=true;try{await queue.waitForIdle().catch(()=>{});await load()}finally{busy.value=false}}
function chooseModel(){perform(async()=>{models.value=(await listAssets(props.projectPath)).models;modelPicker.value=true})}
function attachModel(id:string){perform(async()=>{
  if(!eyeCandy.value)return
  const replacingLegacy=legacyModels.value
  const value=await getModelAsset(id);assets.value[id]=value
  const layer=replacementEyeCandyModel(eyeCandy.value,value)
  eyeCandy.value.models=[layer];selectedLayerId.value=layer.id;selectedPartId.value='';modelPicker.value=false
  await flushEdits()
  if(replacingLegacy){history.reset(eyeCandy.value);historyTick.value++}
})}
async function back(){try{await flush();emit('back')}catch{/* Keep failed edits open. */}}
async function focusIssue(issue:ValidationIssue){if(issue.layerId)selectedLayerId.value=issue.layerId;await nextTick();root.value?.querySelector<HTMLInputElement>(`[data-field="${CSS.escape(issue.field||'')}"]`)?.focus()}
function shortcut(event:KeyboardEvent) {
  if(!(event.ctrlKey||event.metaKey)||event.altKey)return
  const key=event.key.toLowerCase()
  if(key==='s'){event.preventDefault();if(!event.repeat)void flush().catch(()=>{});return}
  if((event.target as HTMLElement)?.closest('input,textarea,[contenteditable="true"]'))return
  if(key==='z'||key==='y'){event.preventDefault();void travelHistory(key==='y'||event.shiftKey?'redo':'undo')}
}
onMounted(()=>{void load();window.addEventListener('keydown',shortcut)})
onBeforeUnmount(()=>{alive=false;clearTimeout(timer);window.removeEventListener('keydown',shortcut)})
defineExpose({flush,focusIssue})
</script>

<template>
  <div ref="root" class="eyeCandy-editor">
    <div v-if="busy" class="eyeCandy-loading"><LoadingProgress :label="eyeCandy?t('loadingAssets'):t('loadingEyeCandy')" /></div>
    <div v-if="loadError" role="alert">{{ loadError }}<button @click="load">{{ t('retry') }}</button></div>
    <template v-if="eyeCandy">
      <p v-if="legacyModels" class="legacy-models" role="status">{{ t('eyeCandyLegacyModels') }}</p>
      <div v-if="saveFailed" class="save-error"><button @click="flush().catch(()=>{})">{{ t('retry') }}</button><button @click="reloadSaved">{{ t('reloadEyeCandy') }}</button></div>
      <header class="eyeCandy-toolbar"><button @click="back"><ArrowLeft :size="16"/>{{ t('decorativeObjects') }}</button><button @click="viewport?.fitView()"><Maximize :size="16"/>{{ t('fitView') }}</button><button :disabled="busy||!canUndo" :title="t('undo')" @click="travelHistory('undo')"><Undo2 :size="16"/></button><button :disabled="busy||!canRedo" :title="t('redo')" @click="travelHistory('redo')"><Redo2 :size="16"/></button><ViewportModeControls v-model:mode="mode" v-model:settings="settings[mode]" v-model:show-grid="grid" v-model:wireframe="wireframe"/><button class="export" @click="emit('export')"><Upload :size="16"/>{{ t('exportPack') }}</button></header>
      <div class="eyeCandy-layout">
        <aside><h2>{{ t('eyeCandyModel') }}</h2><button :disabled="busy" @click="importModel"><Upload :size="15"/>{{ t(selected ? 'replaceModel' : 'import') }}</button><button :disabled="busy" @click="chooseModel">{{ t('chooseFromLibrary') }}</button><div class="layer-list"><button v-for="layer in eyeCandy.models" :key="layer.id" :class="{active:selectedLayerId===layer.id}" @click="select(layer.id)"><component :is="layer.visible?Eye:EyeOff" :size="15"/><span>{{ layer.name }}</span></button></div></aside>
        <main><div class="eyeCandy-viewport"><ModelViewport ref="viewport" :assets="previewLayers" :guides="[]" :ground-height="0" :selected-layer="selectedLayerId" :selected-part="selectedPartId" :selected-instance-key="selectedInstanceKey || selectedLayerId" :render-mode="mode" :settings="settings[mode]" :show-grid="grid" :wireframe="wireframe" @select="select($event.layerId,$event.partId,$event.instanceKey)" @clear-selection="selectedPartId=''" @error="emit('error',$event)" @progress="viewportReady=false" @ready="viewportReady=true"/><div v-if="!viewportReady" class="eyeCandy-preview-loading"><LoadingProgress :label="t('loadingPreview')" /></div><p v-if="!eyeCandy.models.length" class="eyeCandy-empty">{{ t('eyeCandyEmpty') }}</p></div><div class="eyeCandy-tree"><input v-model="query" :placeholder="t('searchParts')"/><ModelHierarchy :storage-key="`eyeCandy-tree:${eyeCandy.id}`" :name="eyeCandy.name" :root-label="t('eyeCandy')" :layers="eyeCandy.models" :assets="assets" :query="query" :selected-layer="selectedLayerId" :selected-part="selectedPartId" @select="select" @visibility="visibility"/></div></main>
        <aside class="eyeCandy-inspector"><fieldset :disabled="busy || legacyModels"><h2>{{ t('general') }}</h2><label>{{ t('eyeCandyName') }}<input v-model="eyeCandy.name" data-field="name" maxlength="80"/></label><label>{{ t('exportId') }}<input v-model="eyeCandy.exportId" data-field="exportId" pattern="[a-z0-9_.-]+"/></label><label>{{ t('description') }}<textarea v-model="eyeCandy.description" rows="3"/></label><label>{{ t('color') }}<input v-model="eyeCandy.color" data-field="color" pattern="[a-fA-F0-9]{6}" maxlength="6"/><small>{{ t('eyeCandyColorHint') }}</small></label>
        <template v-if="selected"><h2>{{ t('modelLayer') }}</h2><label>{{ t('fieldName') }}<input v-model="selected.name"/></label><label class="check"><input v-model="selected.visible" type="checkbox"/>{{ t('visible') }}</label><label class="check"><input v-model="selected.flipTextureV" type="checkbox"/>{{ t('flipV') }}</label><section class="transform-controls"><h2>{{ t('transform') }} · {{ selectedPartId ? t('selectedPart') : t('selectedLayer') }}</h2><p>{{ t('eyeCandyTransformHint') }}</p><div v-for="field in (['translation','rotation','scale'] as const)" :key="field"><span>{{ t(field === 'scale' ? 'scaleRatio' : field) }}</span><div class="transform-axes"><label v-for="(axis,index) in axes" :key="axis">{{ t(axis) }}<input :value="transformValue(field,index)" :data-field="field" type="number" :min="field === 'scale' ? 0.001 : undefined" :step="field === 'rotation' ? 1 : 0.01" @input="changeTransform(field,index,$event)"/></label></div></div><button @click="resetTransform">{{ t('resetTransform') }}</button></section><p v-for="warning in asset?.warnings" :key="warning" class="warning">{{ warning }}</p><MaterialEditor v-for="material in asset?.materials" :key="material.id" :material="material" :binding="selected.materialBindings.find(item=>item.materialId===material.id)" @change="materialChanged(material.id,$event)" @texture="importTexture(material.id,$event)" @pick="pickTexture(material.id,$event)" @reset="selected.materialBindings=selected.materialBindings.filter(item=>item.materialId!==material.id)"/></template></fieldset></aside>
      </div>
      <div v-if="modelPicker" class="eyeCandy-picker" @click.self="modelPicker=false"><section><h2>{{ t('chooseFromLibrary') }}</h2><button v-for="model in models" :key="model.id" @click="attachModel(model.id)">{{ model.name }}</button><p v-if="!models.length">{{ t('noLibraryModels') }}</p><button @click="modelPicker=false">{{ t('cancel') }}</button></section></div>
      <div v-if="texturePicker" class="eyeCandy-picker" @click.self="texturePicker=undefined"><section><h2>{{ t('chooseFromLibrary') }}</h2><button v-for="texture in textures" :key="texture.hash" @click="applyTexture(texture.hash)">{{ texture.name }}</button><p v-if="!textures.length">{{ t('noTexture') }}</p><button @click="texturePicker=undefined">{{ t('cancel') }}</button></section></div>
    </template>
  </div>
</template>

<style scoped>
.eyeCandy-editor{--e-border:var(--studio-border,#ffffff12);--e-text:var(--studio-text,#eef0f3);--e-muted:var(--studio-muted,#92969f);height:100%;min-height:480px;display:flex;flex-direction:column;color:var(--e-text);font:12px var(--studio-font,'Segoe UI',sans-serif);background:var(--studio-bg,#191b1f)}.eyeCandy-toolbar{min-height:48px;display:flex;align-items:center;gap:8px;padding:6px 12px;border-bottom:1px solid var(--e-border)}button{display:inline-flex;align-items:center;justify-content:center;gap:7px;padding:7px 9px;border:1px solid var(--e-border);border-radius:5px;background:#23252a;color:inherit;font:inherit;cursor:pointer}button:hover,button.active{background:#353840}button:disabled{opacity:.4;cursor:default}.eyeCandy-toolbar .export{margin-left:auto;background:#e4e5e8;color:#25272c}.eyeCandy-toolbar label{display:flex;align-items:center;gap:6px}.eyeCandy-toolbar input{width:55px}.eyeCandy-layout{flex:1;min-height:0;display:grid;grid-template-columns:210px minmax(260px,1fr) 290px}.eyeCandy-layout>aside{display:flex;flex-direction:column;gap:8px;padding:14px;border-right:1px solid var(--e-border);overflow:auto}.eyeCandy-layout>main{display:grid;grid-template-rows:minmax(230px,2fr) minmax(160px,1fr);min-height:0;min-width:0}.eyeCandy-layout>.eyeCandy-inspector{border-right:0;border-left:1px solid var(--e-border)}h2{font-size:12px;font-weight:600;margin:8px 0 12px}.layer-list{flex:1;display:flex;flex-direction:column;gap:5px}.layer-list button{justify-content:flex-start;text-align:left;overflow-wrap:anywhere}.eyeCandy-viewport{position:relative;min-height:0}.eyeCandy-tree{overflow:auto;border-top:1px solid var(--e-border);padding:8px}.eyeCandy-tree>input{width:100%;margin-bottom:8px}.eyeCandy-empty{position:absolute;bottom:18px;left:12px;right:12px;text-align:center;pointer-events:none;color:var(--e-muted)}input,textarea{box-sizing:border-box;width:100%;background:#16181c;border:1px solid var(--e-border);border-radius:4px;color:inherit;padding:7px;font:inherit}label{display:grid;gap:6px;margin-bottom:12px;color:var(--e-muted)}.check{display:flex;align-items:center}.check input{width:auto}fieldset{border:0;padding:0;min-width:0}p{color:var(--e-muted);line-height:1.6}.warning{color:#d6bd89}input:focus,textarea:focus,button:focus-visible{outline:1px solid #a5aab5}.eyeCandy-picker{position:fixed;inset:0;background:#0008;z-index:1000;display:grid;place-items:center}.eyeCandy-picker section{background:#23252a;padding:20px;display:grid;gap:8px;max-height:70vh;min-width:300px;overflow:auto}@media(max-width:1100px){.eyeCandy-layout{grid-template-columns:175px minmax(220px,1fr) 250px}.eyeCandy-toolbar label{font-size:10px}}
.eyeCandy-editor{position:relative}.eyeCandy-loading{position:absolute;z-index:9;inset:0;display:grid;place-items:center;background:#191b1feb}.eyeCandy-preview-loading{position:absolute;z-index:3;inset:0;display:grid;place-items:center;background:#191b1feb;pointer-events:none}
</style>

<style scoped>
.legacy-models{margin:0;padding:10px 14px;color:#d6bd89;background:#d6bd890d;border-bottom:1px solid var(--e-border)}
.transform-controls{border-top:1px solid var(--e-border);margin-top:16px;padding-top:8px}.transform-axes{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:6px;margin-top:6px}.transform-axes label{font-size:10px}.transform-axes input{min-width:0;padding:6px 3px}
</style>
