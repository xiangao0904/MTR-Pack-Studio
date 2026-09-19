<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle, ArrowDown, ArrowLeft, ArrowUp, Box, Copy, Eye, EyeOff, Plus, Trash2, Upload } from '@lucide/vue'
import ModelViewport from './ModelViewport.vue'
import { t } from '../i18n'
import { analyzeModelImport, chooseModelDependency, chooseModelFile, getModelAsset, getTrain, importModel, updateTrain, type AssetDefinition, type CarPlacementRule, type CarriageDefinition, type ContentEntry, type ModelLayer, type TrainDefinition } from '../lib/projects'

const props = defineProps<{ projectPath: string; entry: ContentEntry }>()
const emit = defineEmits<{ back: []; changed: [entry: ContentEntry]; status: [value: 'saving' | 'saved' | 'failed']; error: [message: string] }>()
type Tab = 'general' | 'carriages' | 'models' | 'placement' | 'mtr3'
const train = ref<TrainDefinition>()
const selectedCarriageId = ref('')
const selectedLayerId = ref('')
const selectedPartId = ref('')
const tab = ref<Tab>('general')
const assets = ref<Record<string, AssetDefinition>>({})
const importing = ref(false)
let timer: number | undefined
let changeSequence = 0
let hydrating = true
let saving = false

const carriage = computed(() => train.value?.carriages.find(item => item.id === selectedCarriageId.value))
const layers = computed(() => carriage.value ? [...carriage.value.bodyModels, ...carriage.value.bogie1Models, ...carriage.value.bogie2Models] : [])
const selectedLayer = computed(() => layers.value.find(item => item.id === selectedLayerId.value))
const selectedAsset = computed(() => selectedLayer.value ? assets.value[selectedLayer.value.assetId] : undefined)
const viewportAssets = computed(() => layers.value.map(layer => ({ id: layer.assetId, visible: layer.visible })))
const placementRule = computed<CarPlacementRule | undefined>(() => {
  if (!carriage.value) return undefined
  if (selectedPartId.value && selectedLayer.value) return selectedLayer.value.partRules[selectedPartId.value] ||= { preset: 'all', offset: 0, whitelist: '', blacklist: '' }
  return carriage.value.placement
})

async function load() {
  try {
    hydrating = true; train.value = await getTrain(props.projectPath, props.entry.id); selectedCarriageId.value = train.value.carriages[0]?.id || ''
    await nextTick(); await loadAssets(); emit('status', 'saved')
  } catch (cause) { emit('error', message(cause)); emit('status', 'failed') }
  finally { hydrating = false }
}
async function loadAssets() {
  const ids = new Set(layers.value.map(layer => layer.assetId))
  await Promise.all([...ids].filter(id => !assets.value[id]).map(async id => { try { assets.value[id] = await getModelAsset(id) } catch { /* surfaced when selecting the model */ } }))
}
watch(train, () => { if (!hydrating) scheduleSave() }, { deep: true })
watch(selectedCarriageId, () => { selectedLayerId.value = ''; selectedPartId.value = ''; void loadAssets() })
watch(selectedLayerId, () => { selectedPartId.value = ''; void loadAssets() })
onMounted(load)
onBeforeUnmount(() => { if (timer) window.clearTimeout(timer) })

function scheduleSave() { changeSequence += 1; if (timer) window.clearTimeout(timer); timer = window.setTimeout(() => void flush(), 800) }
async function flush() {
  if (!train.value || saving) { if (saving) scheduleSave(); return }
  if (timer) window.clearTimeout(timer); timer = undefined; saving = true; emit('status', 'saving')
  const sequence = changeSequence; const snapshot = JSON.parse(JSON.stringify(train.value)) as TrainDefinition
  try {
    const updated = await updateTrain(props.projectPath, snapshot, snapshot.revision)
    hydrating = true; if (train.value) train.value.revision = updated.revision; await nextTick(); hydrating = false
    emit('changed', { ...props.entry, name: updated.name, updatedAt: Date.now() }); emit('status', 'saved')
    if (sequence !== changeSequence) scheduleSave()
  } catch (cause) { emit('status', 'failed'); emit('error', message(cause)) }
  finally { saving = false }
}
defineExpose({ flush })

function addCarriage() {
  if (!train.value) return
  const number = train.value.carriages.length + 1; const item: CarriageDefinition = { id: crypto.randomUUID(), exportId: `carriage_${number}`, name: `Carriage ${number}`, length: 20, width: 3, bogie1Position: 7, bogie2Position: -7, couplingPadding1: 0, couplingPadding2: 0, end1: { gangway: false, barrier: false }, end2: { gangway: false, barrier: false }, placement: { preset: 'all', offset: 0, whitelist: '', blacklist: '' }, bodyModels: [], bogie1Models: [], bogie2Models: [] }
  train.value.carriages.push(item); train.value.previewConsist.push({ carriageId: item.id, reversed: false }); selectedCarriageId.value = item.id
}
function duplicateCarriage() { if (!train.value || !carriage.value) return; const copy = JSON.parse(JSON.stringify(carriage.value)) as CarriageDefinition; copy.id = crypto.randomUUID(); copy.name += ' Copy'; copy.exportId += '_copy'; for (const layer of [...copy.bodyModels, ...copy.bogie1Models, ...copy.bogie2Models]) layer.id = crypto.randomUUID(); train.value.carriages.push(copy); selectedCarriageId.value = copy.id }
function removeCarriage() { if (!train.value || !carriage.value || train.value.carriages.length === 1) return; const id = carriage.value.id; train.value.carriages = train.value.carriages.filter(item => item.id !== id); train.value.previewConsist = train.value.previewConsist.filter(item => item.carriageId !== id); selectedCarriageId.value = train.value.carriages[0].id }
function moveCarriage(direction: -1 | 1) { if (!train.value || !carriage.value) return; const index = train.value.carriages.indexOf(carriage.value); const next = index + direction; if (next < 0 || next >= train.value.carriages.length) return; [train.value.carriages[index], train.value.carriages[next]] = [train.value.carriages[next], train.value.carriages[index]] }
function selectLayer(layer: ModelLayer) { selectedLayerId.value = layer.id; tab.value = 'models' }
function updateTags(event: Event) { if (train.value) train.value.tags = (event.target as HTMLInputElement).value.split(',').map(value => value.trim()).filter(Boolean) }
async function addModel(slot: 'body' | 'bogie1' | 'bogie2') {
  if (!train.value || !carriage.value) return
  try {
    const path = await chooseModelFile(); if (!path) return; importing.value = true
    const dependencyOverrides: Record<string,string> = {}; let analysis = await analyzeModelImport(path, dependencyOverrides)
    while (analysis.missingDependencies.length) { for (const missing of analysis.missingDependencies) { const name = missing.split(/[\\/]/).pop() || missing; const resolved = await chooseModelDependency(name); if (!resolved) throw new Error(`${t('missingDependencies')}: ${name}`); dependencyOverrides[name] = resolved } analysis = await analyzeModelImport(path, dependencyOverrides) }
    const result = await importModel(train.value.id, carriage.value.id, slot, path, dependencyOverrides); hydrating = true; train.value = result.train; assets.value[result.asset.id] = result.asset; selectedLayerId.value = [...carriage.value.bodyModels, ...carriage.value.bogie1Models, ...carriage.value.bogie2Models].at(-1)?.id || ''; await nextTick(); hydrating = false; emit('status', 'saved')
  } catch (cause) { emit('error', message(cause)); emit('status', 'failed') } finally { importing.value = false }
}
function message(cause: unknown) { return cause instanceof Error ? cause.message : String(cause) }
</script>

<template>
  <div v-if="train" class="train-editor">
    <header class="editor-header"><button @click="emit('back')"><ArrowLeft :size="17" />{{ t('trains') }}</button><div><strong>{{ train.name }}</strong><span>{{ train.exportId }}</span></div><nav><button v-for="item in (['general','carriages','models','placement','mtr3'] as Tab[])" :key="item" :class="{active:tab===item}" @click="tab=item">{{ t(item === 'mtr3' ? 'mtr3Compatibility' : item) }}</button></nav></header>
    <div class="editor-body">
      <aside class="carriage-list"><div class="panel-title"><span>{{ t('carriages') }}</span><button @click="addCarriage"><Plus :size="16" /></button></div><button v-for="(item,index) in train.carriages" :key="item.id" :class="['carriage-item',{active:item.id===selectedCarriageId}]" @click="selectedCarriageId=item.id"><Box :size="17" /><span><strong>{{ item.name }}</strong><small>{{ index + 1 }} · {{ item.length }} m</small></span></button><div class="carriage-actions"><button @click="moveCarriage(-1)"><ArrowUp :size="15" /></button><button @click="moveCarriage(1)"><ArrowDown :size="15" /></button><button @click="duplicateCarriage"><Copy :size="15" /></button><button :disabled="train.carriages.length===1" @click="removeCarriage"><Trash2 :size="15" /></button></div></aside>
      <section class="viewport-panel"><ModelViewport :assets="viewportAssets" :selected-part="selectedPartId" @select="selectedPartId=$event;tab='placement'" /><div v-if="selectedAsset?.warnings.length" class="model-warning"><AlertTriangle :size="15" />{{ selectedAsset.warnings.join(' ') }}</div></section>
      <aside class="inspector">
        <template v-if="tab==='general'"><h2>{{ t('general') }}</h2><label>{{ t('trainName') }}<input v-model="train.name" maxlength="80" /></label><label>{{ t('exportId') }}<input v-model="train.exportId" /></label><label>{{ t('description') }}<textarea v-model="train.description" rows="4" /></label><label>{{ t('color') }}<span class="color-row"><input v-model="train.color" /><input type="color" :value="`#${train.color}`" @input="train.color=($event.target as HTMLInputElement).value.slice(1).toUpperCase()" /></span></label><label>{{ t('tags') }}<input :value="train.tags.join(', ')" @change="updateTags" /></label></template>
        <template v-else-if="tab==='carriages' && carriage"><h2>{{ t('carriages') }}</h2><label>{{ t('name') }}<input v-model="carriage.name" /></label><label>{{ t('exportId') }}<input v-model="carriage.exportId" /></label><div class="field-grid"><label>{{ t('length') }}<input v-model.number="carriage.length" type="number" step="0.1" /></label><label>{{ t('width') }}<input v-model.number="carriage.width" type="number" step="0.1" /></label><label>{{ t('bogie1') }}<input v-model.number="carriage.bogie1Position" type="number" step="0.1" /></label><label>{{ t('bogie2') }}<input v-model.number="carriage.bogie2Position" type="number" step="0.1" /></label></div><h3>{{ t('endConnections') }}</h3><div class="check-grid"><label><input v-model="carriage.end1.gangway" type="checkbox" />{{ t('end1Gangway') }}</label><label><input v-model="carriage.end2.gangway" type="checkbox" />{{ t('end2Gangway') }}</label><label><input v-model="carriage.end1.barrier" type="checkbox" />{{ t('end1Barrier') }}</label><label><input v-model="carriage.end2.barrier" type="checkbox" />{{ t('end2Barrier') }}</label></div></template>
        <template v-else-if="tab==='models' && carriage"><h2>{{ t('models') }}</h2><div class="import-buttons"><button :disabled="importing" @click="addModel('body')"><Upload :size="15" />{{ t('bodyModel') }}</button><button :disabled="importing" @click="addModel('bogie1')">{{ t('bogie1') }}</button><button :disabled="importing" @click="addModel('bogie2')">{{ t('bogie2') }}</button></div><div v-for="layer in layers" :key="layer.id" :class="['layer-row',{active:layer.id===selectedLayerId}]" @click="selectLayer(layer)"><button @click.stop="layer.visible=!layer.visible"><component :is="layer.visible?Eye:EyeOff" :size="15" /></button><span>{{ layer.name }}</span><small>{{ assets[layer.assetId]?.sourceFormat?.toUpperCase() }}</small></div><div v-if="selectedAsset" class="parts"><h3>{{ t('parts') }}</h3><button v-for="part in selectedAsset.parts" :key="part.id" :class="{active:part.id===selectedPartId}" @click="selectedPartId=part.id;tab='placement'"><span>{{ part.name }}</span><small>{{ part.triangleCount }} △</small></button></div></template>
        <template v-else-if="tab==='placement' && placementRule"><h2>{{ selectedPartId ? t('partPlacement') : t('carriagePlacement') }}</h2><label>{{ t('preset') }}<select v-model="placementRule.preset"><option value="all">{{ t('allCars') }}</option><option value="first">{{ t('firstCar') }}</option><option value="last">{{ t('lastCar') }}</option><option value="odd">{{ t('oddCars') }}</option><option value="even">{{ t('evenCars') }}</option><option value="every">{{ t('everyCars') }}</option><option value="custom">{{ t('custom') }}</option></select></label><div v-if="placementRule.preset==='every'" class="field-grid"><label>{{ t('interval') }}<input v-model.number="placementRule.every" type="number" min="1" /></label><label>{{ t('offset') }}<input v-model.number="placementRule.offset" type="number" /></label></div><template v-if="placementRule.preset==='custom'"><label>{{ t('whitelist') }}<input v-model="placementRule.whitelist" placeholder="1,-1,%2" /></label><label>{{ t('blacklist') }}<input v-model="placementRule.blacklist" placeholder="2,%4" /></label></template></template>
        <template v-else-if="tab==='mtr3'"><h2>{{ t('mtr3Compatibility') }}</h2><p class="inspector-hint">{{ t('mtr3Hint') }}</p><label>{{ t('baseTrainType') }}<input v-model="train.mtr3BaseTrainType" placeholder="sp1900" /></label><div class="compat-note"><AlertTriangle :size="17" /><span>{{ t('mtr3LengthHint') }}</span></div></template>
      </aside>
    </div>
  </div>
  <div v-else class="editor-loading">{{ t('loading') }}</div>
</template>

<style scoped>
.train-editor{height:100%;min-height:0;display:flex;flex-direction:column;border:1px solid #37414a;border-radius:9px;overflow:hidden;background:#171c20}.editor-header{height:56px;flex:none;display:flex;align-items:center;border-bottom:1px solid #37414a;background:#22282e;padding:0 12px;gap:15px}.editor-header>button{display:flex;align-items:center;gap:7px;border:0;background:none;color:#cbd4dd}.editor-header>div{display:flex;flex-direction:column;min-width:150px}.editor-header strong{font-size:13px}.editor-header span{font-size:10px;color:#8995a0;margin-top:3px}.editor-header nav{display:flex;align-self:stretch;margin-left:auto}.editor-header nav button{border:0;border-bottom:2px solid transparent;background:none;color:#9da8b3;padding:0 12px;font-size:11px}.editor-header nav button.active{color:#fff;border-color:#dcecff}.editor-body{flex:1;min-height:0;display:grid;grid-template-columns:190px minmax(320px,1fr) 290px}.carriage-list,.inspector{overflow:auto;background:#20262b}.carriage-list{border-right:1px solid #37414a;padding:12px}.panel-title{display:flex;align-items:center;justify-content:space-between;color:#aeb9c3;font-size:11px;text-transform:uppercase;letter-spacing:.09em;margin:3px 3px 10px}.panel-title button,.carriage-actions button{border:1px solid #46515b;background:#2d353d;color:#dfe6ed;border-radius:5px;padding:5px}.carriage-item{width:100%;display:flex;align-items:center;gap:9px;border:1px solid transparent;border-radius:6px;background:none;color:#d3dbe3;text-align:left;padding:9px}.carriage-item:hover,.carriage-item.active{background:#303942;border-color:#4d5965}.carriage-item span{min-width:0}.carriage-item strong,.carriage-item small{display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.carriage-item strong{font-size:11px}.carriage-item small{font-size:9px;color:#8f9ba7;margin-top:3px}.carriage-actions{display:flex;gap:5px;margin-top:10px}.viewport-panel{min-width:0;min-height:0;position:relative}.model-warning{position:absolute;left:12px;right:12px;bottom:12px;display:flex;gap:8px;align-items:center;padding:9px 11px;border:1px solid #80683c;border-radius:6px;background:#3b3325dd;color:#f1d59c;font-size:10px}.inspector{border-left:1px solid #37414a;padding:18px}.inspector h2{font:600 18px Outfit,sans-serif;margin:0 0 18px}.inspector h3,.parts h3{font-size:11px;color:#aeb8c2;text-transform:uppercase;letter-spacing:.08em;margin:20px 0 9px}.inspector label{display:flex;flex-direction:column;gap:6px;color:#abb6c0;font-size:10px;margin-bottom:12px}.inspector input,.inspector textarea,.inspector select{width:100%;border:1px solid #4a5661;border-radius:5px;background:#171c20;color:#f1f4f7;padding:8px;outline:0;font-size:11px}.inspector textarea{resize:vertical}.field-grid{display:grid;grid-template-columns:1fr 1fr;gap:9px}.check-grid{display:grid;grid-template-columns:1fr 1fr;gap:8px}.check-grid label{display:flex;flex-direction:row;align-items:center;margin:0}.check-grid input{width:auto}.color-row{display:flex;gap:7px}.color-row input[type=color]{width:38px;padding:2px}.import-buttons{display:grid;grid-template-columns:1fr 1fr;gap:6px;margin-bottom:14px}.import-buttons button{min-height:34px;border:1px solid #52606c;border-radius:5px;background:#303943;color:#dce4eb;font-size:10px;display:flex;align-items:center;justify-content:center;gap:5px}.import-buttons button:first-child{grid-column:1/-1}.layer-row{display:flex;align-items:center;gap:7px;padding:8px;border:1px solid transparent;border-radius:5px;font-size:11px;cursor:pointer}.layer-row.active{border-color:#607080;background:#2c353d}.layer-row button{border:0;background:none;color:#bac5cf;padding:0}.layer-row small{margin-left:auto;color:#84919d;font-size:8px}.parts button{width:100%;display:flex;justify-content:space-between;border:0;border-radius:4px;background:none;color:#bdc7d0;padding:7px;font-size:10px}.parts button.active,.parts button:hover{background:#35404a;color:#fff}.parts small{color:#86939f}.inspector-hint{color:#939faa;font-size:11px;line-height:1.5}.compat-note{display:flex;gap:9px;color:#d8bf8e;background:#353025;border-radius:6px;padding:10px;font-size:10px;line-height:1.4}.editor-loading{height:400px;display:grid;place-items:center;color:#99a5af}@media(max-width:1150px){.editor-body{grid-template-columns:155px minmax(260px,1fr) 250px}.editor-header nav button{padding:0 7px}.editor-header>div{display:none}}
</style>
