<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ArrowLeft, ArrowUp, ArrowDown, Copy, Eye, EyeOff, Maximize, Plus, Trash2, Upload } from '@lucide/vue'
import { t } from '../i18n'
import ModelViewport, { type PreviewLayer } from './ModelViewport.vue'
import ModelHierarchy from './ModelHierarchy.vue'
import MaterialEditor from './MaterialEditor.vue'
import ViewportModeControls from './ViewportModeControls.vue'
import { defaultViewportSettings, type PreviewRenderMode } from '../lib/viewport-settings'
import { SaveQueue } from '../lib/save-queue'
import { analyzeModelImport, chooseModelDependency, chooseModelFile, chooseTextureFile, deleteRail, getRail, getModelAsset, importRailModel, importTextureFile, listAssets, updateRail, type AssetDefinition, type ContentEntry, type ModelCatalogItem, type MaterialBinding, type MaterialProperties, type ModelLayer, type RailDefinition, type TextureChannel, type TextureCatalogItem, type ValidationIssue } from '../lib/projects'

const props = defineProps<{ projectPath: string; entry: ContentEntry }>()
const emit = defineEmits<{ back: []; deleted: []; export: []; changed: [entry: ContentEntry]; status: [value: 'saving'|'saved'|'failed']; error: [message: string]; ready: [] }>()
const rail = ref<RailDefinition>()
const assets = ref<Record<string, AssetDefinition>>({})
const selectedLayerId = ref(''), selectedPartId = ref(''), selectedInstanceKey = ref(''), query = ref('')
const busy = ref(false), loadError = ref(''), repeats = ref(9)
const viewport = ref<InstanceType<typeof ModelViewport>>()
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
const selected = computed(() => rail.value?.models.find(layer => layer.id === selectedLayerId.value))
const asset = computed(() => selected.value ? assets.value[selected.value.assetId] : undefined)
const count = computed(() => Math.min(50, Math.max(1, Math.floor(Number(repeats.value) || 1))))
const previewLayers = computed<PreviewLayer[]>(() => {
  if (!rail.value) return []
  const interval = Number.isFinite(rail.value.repeatInterval) && rail.value.repeatInterval > 0 ? rail.value.repeatInterval : .6
  return Array.from({length: count.value}, (_, index) => rail.value!.models.map(layer => ({ key:`${index}:${layer.id}`, carriageId:rail.value!.id, layerId:layer.id, assetId:layer.assetId, visible:layer.visible, z:(index-(count.value-1)/2)*interval, reversed:false, bogieOffset:0, flipV:layer.flipTextureV, legacyUvCorrection:assets.value[layer.assetId]?.legacyUvCorrection, bindings:layer.materialBindings, hiddenParts:layer.hiddenParts||[], transform:layer.transform, partTransforms:layer.partTransforms, renderStage:layer.renderStage, partRenderStages:layer.partRenderStages }))).flat()
})
async function writeSnapshot() {
  if (!rail.value) return
  const snapshot: RailDefinition = JSON.parse(JSON.stringify(rail.value))
  const saved = await updateRail(props.projectPath, snapshot, snapshot.revision)
  hydrating = true; rail.value.revision = saved.revision; hydrating = false
  emit('changed', {...props.entry, name:saved.name, updatedAt:Date.now()})
}
let queue = new SaveQueue(writeSnapshot)
function report(cause: unknown) { saveFailed.value=true; emit('status','failed'); emit('error', cause instanceof Error ? cause.message : String(cause)) }
async function load() {
  busy.value=true
  try {
    hydrating=true; rail.value=await getRail(props.projectPath,props.entry.id)
    await loadAssets(); selectedLayerId.value=rail.value.models[0]?.id||''; loadError.value=''
    queue=new SaveQueue(writeSnapshot);saveFailed.value=false;emit('error','');emit('status','saved'); await nextTick(); emit('ready')
  } catch(cause) { loadError.value=String(cause); report(cause) } finally { hydrating=false;busy.value=false }
}
async function loadAssets() { for (const layer of rail.value?.models||[]) if (!assets.value[layer.assetId]) assets.value[layer.assetId]=await getModelAsset(layer.assetId) }
watch(rail, () => { if(hydrating)return; queue.markDirty(); emit('status','saving'); clearTimeout(timer); timer=setTimeout(()=>void flush().catch(()=>{}),800) }, {deep:true,flush:'sync'})
async function flushEdits() { clearTimeout(timer); try { await queue.flush(); saveFailed.value=false;emit('status','saved') } catch(cause) { report(cause); throw cause } }
async function flush() { if(operation)await operation; await flushEdits() }
function perform(action: () => Promise<void>) {
  if(busy.value)return
  busy.value=true
  operation=(async()=>{try{await flushEdits();await action()}catch(cause){report(cause);throw cause}finally{busy.value=false;operation=undefined}})()
  void operation.catch(()=>{})
}
function importModel(useDefault: boolean) { perform(async()=>{
  if(!rail.value)return
  const path=useDefault ? null : await chooseModelFile(); if(!useDefault&&!path)return
  const overrides: Record<string,string>={}
  if(path) {
    const analysis=await analyzeModelImport(path)
    for(const dependency of analysis.missingDependencies){const chosen=await chooseModelDependency(dependency);if(!chosen)return;overrides[dependency]=chosen}
  }
  emit('status','saving')
  const result=await importRailModel(props.projectPath,rail.value.id,path,overrides,rail.value.revision)
  if(!alive)return
  hydrating=true;rail.value=result.rail;assets.value[result.asset.id]=result.asset;hydrating=false
  selectedLayerId.value=result.rail.models.at(-1)?.id||'';selectedPartId.value=''
  emit('changed',{...props.entry,name:result.rail.name,updatedAt:Date.now()});emit('status','saved')
}) }
function modify(action:()=>void) { perform(async()=>{action();await flushEdits()}) }
function move(delta:number) { modify(()=>{const models=rail.value!.models,index=models.findIndex(layer=>layer.id===selectedLayerId.value),next=index+delta;if(index>=0&&next>=0&&next<models.length)[models[index],models[next]]=[models[next]!,models[index]!]}) }
function duplicate() { modify(()=>{if(!selected.value)return;const copy:ModelLayer=JSON.parse(JSON.stringify(selected.value));copy.id=crypto.randomUUID();rail.value!.models.push(copy);selectedLayerId.value=copy.id}) }
function removeLayer() { modify(()=>{rail.value!.models=rail.value!.models.filter(layer=>layer.id!==selectedLayerId.value);selectedLayerId.value=rail.value!.models[0]?.id||'';selectedPartId.value=''}) }
function visibility(id:string|null,parts?:string[]) { modify(()=>{
  const layers=id?rail.value!.models.filter(layer=>layer.id===id):rail.value!.models
  if(parts)for(const layer of layers){const hidden=new Set(layer.hiddenParts||[]),hide=parts.some(part=>!hidden.has(part));for(const part of parts)hide?hidden.add(part):hidden.delete(part);layer.hiddenParts=[...hidden]}
  else {const visible=!layers.some(layer=>layer.visible);for(const layer of layers)layer.visible=visible}
}) }
function select(layerId:string,partId='',instanceKey=''){selectedLayerId.value=layerId;selectedPartId.value=partId;selectedInstanceKey.value=instanceKey}
watch([selectedLayerId,count],()=>{selectedInstanceKey.value=''},{flush:'sync'})
function binding(layer:ModelLayer,id:string):MaterialBinding {let value=layer.materialBindings.find(item=>item.materialId===id);if(!value){value={materialId:id};layer.materialBindings.push(value)}return value}
function materialChanged(id:string,properties:MaterialProperties){if(selected.value)binding(selected.value,id).properties=properties}
function setTexture(layer:ModelLayer,id:string,hash:string,channel?:TextureChannel){const value=binding(layer,id);if(channel)value.properties={...value.properties,maps:{...value.properties?.maps,[channel]:hash}};else value.textureAssetId=hash}
function importTexture(id:string,channel?:TextureChannel){perform(async()=>{const layer=selected.value;if(!layer)return;const path=await chooseTextureFile();if(!path)return;setTexture(layer,id,await importTextureFile(path),channel);await flushEdits()})}
function pickTexture(id:string,channel?:TextureChannel){perform(async()=>{textures.value=(await listAssets(props.projectPath)).textures;texturePicker.value={materialId:id,channel,layerId:selectedLayerId.value}})}
function applyTexture(hash:string){modify(()=>{const request=texturePicker.value,layer=rail.value?.models.find(item=>item.id===request?.layerId);if(request&&layer)setTexture(layer,request.materialId,hash,request.channel);texturePicker.value=undefined})}
async function reloadSaved(){if(busy.value||!window.confirm(t('railReloadHint')))return;clearTimeout(timer);busy.value=true;try{await queue.waitForIdle().catch(()=>{});await load()}finally{busy.value=false}}
function chooseModel(){perform(async()=>{models.value=(await listAssets(props.projectPath)).models;modelPicker.value=true})}
function attachModel(id:string){perform(async()=>{if(!rail.value)return;const value=await getModelAsset(id);assets.value[id]=value;const layer:ModelLayer={id:crypto.randomUUID(),name:value.name,assetId:id,visible:true,flipTextureV:false,materialBindings:[],partRules:{}};rail.value.models.push(layer);selectedLayerId.value=layer.id;modelPicker.value=false;await flushEdits()})}
function removeRail(){if(!window.confirm(t('confirmDeleteRail')))return;perform(async()=>{await deleteRail(props.projectPath,props.entry.id);emit('deleted')})}
async function back(){try{await flush();emit('back')}catch{/* Keep failed edits open. */}}
async function focusIssue(issue:ValidationIssue){if(issue.layerId)selectedLayerId.value=issue.layerId;await nextTick();root.value?.querySelector<HTMLInputElement>(`[data-field="${CSS.escape(issue.field||'')}"]`)?.focus()}
function shortcut(event:KeyboardEvent){if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='s'){event.preventDefault();void flush().catch(()=>{})}}
onMounted(()=>{void load();window.addEventListener('keydown',shortcut)})
onBeforeUnmount(()=>{alive=false;clearTimeout(timer);window.removeEventListener('keydown',shortcut)})
defineExpose({flush,focusIssue})
</script>

<template>
  <div ref="root" class="rail-editor">
    <div v-if="loadError" role="alert">{{ loadError }}<button @click="load">{{ t('retry') }}</button></div>
    <template v-if="rail">
      <div v-if="saveFailed" class="save-error"><button @click="flush().catch(()=>{})">{{ t('retry') }}</button><button @click="reloadSaved">{{ t('reloadRail') }}</button></div>
      <header class="rail-toolbar"><button @click="back"><ArrowLeft :size="16"/>{{ t('rails') }}</button><button @click="viewport?.fitView()"><Maximize :size="16"/>{{ t('fitView') }}</button><ViewportModeControls v-model:mode="mode" v-model:settings="settings[mode]" v-model:show-grid="grid" v-model:wireframe="wireframe"/><label>{{ t('railPreviewRepeats') }}<input v-model.number="repeats" type="number" min="1" max="50"/></label><button class="export" @click="emit('export')"><Upload :size="16"/>{{ t('exportPack') }}</button></header>
      <div class="rail-layout">
        <aside><h2>{{ t('railModels') }}</h2><button :disabled="busy" @click="importModel(true)"><Plus :size="15"/>{{ t('defaultRailModel') }}</button><button :disabled="busy" @click="importModel(false)"><Upload :size="15"/>{{ t('import') }}</button><button :disabled="busy" @click="chooseModel">{{ t('chooseFromLibrary') }}</button><div class="layer-list"><button v-for="layer in rail.models" :key="layer.id" :class="{active:selectedLayerId===layer.id}" @click="select(layer.id)"><component :is="layer.visible?Eye:EyeOff" :size="15"/><span>{{ layer.name }}</span></button></div><div class="layer-actions"><button :disabled="busy||!selected" :title="t('moveUp')" :aria-label="t('moveUp')" @click="move(-1)"><ArrowUp :size="15"/></button><button :disabled="busy||!selected" :title="t('moveDown')" :aria-label="t('moveDown')" @click="move(1)"><ArrowDown :size="15"/></button><button :disabled="busy||!selected" :title="t('duplicate')" :aria-label="t('duplicate')" @click="duplicate"><Copy :size="15"/></button><button :disabled="busy||!selected" :title="t('delete')" :aria-label="t('delete')" @click="removeLayer"><Trash2 :size="15"/></button></div><button :disabled="busy" @click="removeRail"><Trash2 :size="15"/>{{ t('deleteRail') }}</button></aside>
        <main><div class="rail-viewport"><ModelViewport ref="viewport" :assets="previewLayers" :guides="[]" :ground-height="0" :selected-layer="selectedLayerId" :selected-part="selectedPartId" :selected-instance-key="selectedInstanceKey || `${Math.floor((count-1)/2)}:${selectedLayerId}`" :render-mode="mode" :settings="settings[mode]" :show-grid="grid" :wireframe="wireframe" @select="select($event.layerId,$event.partId,$event.instanceKey)" @clear-selection="selectedPartId=''" @error="emit('error',$event)"/><p v-if="!rail.models.length" class="rail-empty">{{ t('railEmpty') }}</p></div><div class="rail-tree"><input v-model="query" :placeholder="t('searchParts')"/><ModelHierarchy :storage-key="`rail-tree:${rail.id}`" :name="rail.name" :root-label="t('rail')" :layers="rail.models" :assets="assets" :query="query" :selected-layer="selectedLayerId" :selected-part="selectedPartId" @select="select" @visibility="visibility"/></div></main>
        <aside class="rail-inspector"><fieldset :disabled="busy"><h2>{{ t('general') }}</h2><label>{{ t('railName') }}<input v-model="rail.name" data-field="name" maxlength="80"/></label><label>{{ t('exportId') }}<input v-model="rail.exportId" data-field="exportId" pattern="[a-z0-9_.-]+"/></label><label>{{ t('description') }}<textarea v-model="rail.description" rows="3"/></label><label>{{ t('railRepeatInterval') }}<input v-model.number="rail.repeatInterval" data-field="repeatInterval" type="number" min="0.01" max="100" step="0.01"/></label><p>{{ t('railRepeatHint') }}</p>
        <template v-if="selected"><h2>{{ t('modelLayer') }}</h2><label>{{ t('fieldName') }}<input v-model="selected.name"/></label><label class="check"><input v-model="selected.visible" type="checkbox"/>{{ t('visible') }}</label><label class="check"><input v-model="selected.flipTextureV" type="checkbox"/>{{ t('flipV') }}</label><p v-for="warning in asset?.warnings" :key="warning" class="warning">{{ warning }}</p><MaterialEditor v-for="material in asset?.materials" :key="material.id" :material="material" :binding="selected.materialBindings.find(item=>item.materialId===material.id)" @change="materialChanged(material.id,$event)" @texture="importTexture(material.id,$event)" @pick="pickTexture(material.id,$event)" @reset="selected.materialBindings=selected.materialBindings.filter(item=>item.materialId!==material.id)"/></template></fieldset></aside>
      </div>
      <div v-if="modelPicker" class="rail-picker" @click.self="modelPicker=false"><section><h2>{{ t('chooseFromLibrary') }}</h2><button v-for="model in models" :key="model.id" @click="attachModel(model.id)">{{ model.name }}</button><p v-if="!models.length">{{ t('noLibraryModels') }}</p><button @click="modelPicker=false">{{ t('cancel') }}</button></section></div>
      <div v-if="texturePicker" class="rail-picker" @click.self="texturePicker=undefined"><section><h2>{{ t('chooseFromLibrary') }}</h2><button v-for="texture in textures" :key="texture.hash" @click="applyTexture(texture.hash)">{{ texture.name }}</button><p v-if="!textures.length">{{ t('noTexture') }}</p><button @click="texturePicker=undefined">{{ t('cancel') }}</button></section></div>
    </template>
  </div>
</template>

<style scoped>
.rail-editor{--e-border:var(--studio-border,#ffffff12);--e-text:var(--studio-text,#eef0f3);--e-muted:var(--studio-muted,#92969f);height:100%;min-height:480px;display:flex;flex-direction:column;color:var(--e-text);font:12px var(--studio-font,'Segoe UI',sans-serif);background:var(--studio-bg,#191b1f)}.rail-toolbar{min-height:48px;display:flex;align-items:center;gap:8px;padding:6px 12px;border-bottom:1px solid var(--e-border)}button{display:inline-flex;align-items:center;justify-content:center;gap:7px;padding:7px 9px;border:1px solid var(--e-border);border-radius:5px;background:#23252a;color:inherit;font:inherit;cursor:pointer}button:hover,button.active{background:#353840}button:disabled{opacity:.4;cursor:default}.rail-toolbar .export{margin-left:auto;background:#e4e5e8;color:#25272c}.rail-toolbar label{display:flex;align-items:center;gap:6px}.rail-toolbar input{width:55px}.rail-layout{flex:1;min-height:0;display:grid;grid-template-columns:210px minmax(260px,1fr) 290px}.rail-layout>aside{display:flex;flex-direction:column;gap:8px;padding:14px;border-right:1px solid var(--e-border);overflow:auto}.rail-layout>main{display:grid;grid-template-rows:minmax(230px,2fr) minmax(160px,1fr);min-height:0;min-width:0}.rail-layout>.rail-inspector{border-right:0;border-left:1px solid var(--e-border)}h2{font-size:12px;font-weight:600;margin:8px 0 12px}.layer-list{flex:1;display:flex;flex-direction:column;gap:5px}.layer-list button{justify-content:flex-start;text-align:left;overflow-wrap:anywhere}.layer-actions{display:flex;gap:5px}.rail-viewport{position:relative;min-height:0}.rail-tree{overflow:auto;border-top:1px solid var(--e-border);padding:8px}.rail-tree>input{width:100%;margin-bottom:8px}.rail-empty{position:absolute;bottom:18px;left:12px;right:12px;text-align:center;pointer-events:none;color:var(--e-muted)}input,textarea{box-sizing:border-box;width:100%;background:#16181c;border:1px solid var(--e-border);border-radius:4px;color:inherit;padding:7px;font:inherit}label{display:grid;gap:6px;margin-bottom:12px;color:var(--e-muted)}.check{display:flex;align-items:center}.check input{width:auto}fieldset{border:0;padding:0;min-width:0}p{color:var(--e-muted);line-height:1.6}.warning{color:#d6bd89}input:focus,textarea:focus,button:focus-visible{outline:1px solid #a5aab5}.rail-picker{position:fixed;inset:0;background:#0008;z-index:1000;display:grid;place-items:center}.rail-picker section{background:#23252a;padding:20px;display:grid;gap:8px;max-height:70vh;min-width:300px;overflow:auto}@media(max-width:1100px){.rail-layout{grid-template-columns:175px minmax(220px,1fr) 250px}.rail-toolbar label{font-size:10px}}
</style>
