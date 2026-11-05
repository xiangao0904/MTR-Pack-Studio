<script setup lang="ts">
import { computed, onBeforeUnmount } from 'vue'
import { t } from '../i18n'
import type { MaterialBinding, MaterialProperties, ModelMaterial, TextureChannel } from '../lib/projects'

const props = defineProps<{ material: ModelMaterial; binding?: MaterialBinding }>()
const emit = defineEmits<{ change: [properties: MaterialProperties, commit?: boolean]; texture: [channel?: TextureChannel]; pick: [channel?: TextureChannel]; reset: [] }>()
const effective = computed(() => {
  const properties: MaterialProperties = {...props.material.properties}
  for(const [key,value] of Object.entries(props.binding?.properties || {}))if(value!=null)Object.assign(properties,{[key]:value})
  return properties
})
const channels = ['labNormal','labSpecular','normal','metalness','roughness','emissive','occlusion'] as const
const channelLabels = {labNormal:'labNormalMap',labSpecular:'labSpecularMap',normal:'normalMap',metalness:'metalnessMap',roughness:'roughnessMap',emissive:'emissiveMap',occlusion:'occlusionMap'} as const
const pbrLocked = computed(() => channels.some(channel => !!(props.binding?.properties?.maps?.[channel] || props.material.properties?.maps?.[channel])))
const value = (key: 'metalness'|'roughness'|'opacity'|'normalScale'|'alphaCutoff') => effective.value[key] ?? ({metalness:0,roughness:.8,opacity:props.material.color[3],normalScale:1,alphaCutoff:.1}[key])
function change<K extends keyof MaterialProperties>(key: K, value: MaterialProperties[K], commit=true) { emit('change',{...props.binding?.properties,[key]:value},commit) }
type NumericProperty='metalness'|'roughness'|'opacity'|'normalScale'|'alphaCutoff'
let pendingFrame:number|undefined
let pendingValue:{key:NumericProperty;value:number}|undefined
function number(event: Event,key:NumericProperty,final=false) {
  if(pbrLocked.value && key!=='opacity' && key!=='alphaCutoff')return
  const value=Number((event.target as HTMLInputElement).value)
  if(!Number.isFinite(value))return
  if(final){if(pendingFrame!==undefined)cancelAnimationFrame(pendingFrame);pendingFrame=undefined;pendingValue=undefined;change(key,value);return}
  pendingValue={key,value}
  if(pendingFrame===undefined)pendingFrame=requestAnimationFrame(()=>{pendingFrame=undefined;const pending=pendingValue;pendingValue=undefined;if(pending)change(pending.key,pending.value,false)})
}
onBeforeUnmount(()=>{if(pendingFrame!==undefined)cancelAnimationFrame(pendingFrame)})
function mapName(channel: TextureChannel) {return props.binding?.properties?.maps?.[channel] ? t('customTexture') : props.material.properties?.maps?.[channel]?.split(/[\\/]/).pop() || t('noTexture')}
const emission = computed(() => '#' + (effective.value.emissive || [0,0,0]).map(v=>Math.round(Math.min(1,Math.max(0,v))*255).toString(16).padStart(2,'0')).join(''))
function setEmission(event: Event) {const hex=(event.target as HTMLInputElement).value;change('emissive',[1,3,5].map(offset=>parseInt(hex.slice(offset,offset+2),16)/255) as [number,number,number])}
const alphaMode = computed(() => effective.value.alphaMode || (value('opacity')<1 ? 'BLEND' : props.material.texture || props.binding?.textureAssetId ? 'MASK' : 'OPAQUE'))
</script>

<template>
  <details class="material-editor">
    <summary><span class="swatch" :style="{background:`rgba(${material.color.slice(0,3).map(v=>Math.round(v*255)).join(',')},${material.color[3]})`}"/>{{ material.name }}</summary>
    <div class="material-fields">
      <p>{{ t('materialPreviewHint') }}</p><p v-if="pbrLocked">{{ t('pbrTextureLocked') }}</p>
      <div class="texture"><span>{{ t('baseColorMap') }}<small>{{ binding?.textureAssetId ? t('customTexture') : material.texture || t('solidColor') }}</small></span><button @click="emit('texture')">{{ t('import') }}</button><button @click="emit('pick')">{{ t('chooseFromLibrary') }}</button></div>
      <label v-for="key in (['metalness','roughness','opacity'] as const)" :key="key"><span>{{ t(key) }}<output>{{ value(key).toFixed(2) }}</output></span><input type="range" min="0" max="1" :step="key==='metalness'?1:0.01" :disabled="pbrLocked && key!=='opacity'" :aria-label="`${material.name} ${t(key)}`" :value="value(key)" @input="number($event,key)" @change="number($event,key,true)"/></label>
      <label class="inline">{{ t('emissiveColor') }}<input type="color" :disabled="pbrLocked" :value="emission" @change="setEmission"/></label>
      <label class="inline">{{ t('alphaMode') }}<select :value="alphaMode" @change="change('alphaMode',($event.target as HTMLSelectElement).value as MaterialProperties['alphaMode'])"><option value="OPAQUE">{{ t('alphaOpaque') }}</option><option value="MASK">{{ t('alphaMask') }}</option><option value="BLEND">{{ t('alphaBlend') }}</option></select></label>
      <label v-if="alphaMode==='MASK'"><span>{{ t('alphaCutoff') }}<output>{{ value('alphaCutoff').toFixed(2) }}</output></span><input type="range" min="0" max="1" step="0.01" :value="value('alphaCutoff')" @input="number($event,'alphaCutoff')" @change="number($event,'alphaCutoff',true)"/></label>
      <label class="inline">{{ t('doubleSided') }}<input type="checkbox" :checked="effective.doubleSided ?? true" @change="change('doubleSided',($event.target as HTMLInputElement).checked)"/></label>
      <div v-for="channel in channels" :key="channel" class="texture"><span>{{ t(channelLabels[channel]) }}<small>{{ mapName(channel) }}</small></span><button @click="emit('texture',channel)">{{ t('import') }}</button><button @click="emit('pick',channel)">{{ t('chooseFromLibrary') }}</button></div>
      <label><span>{{ t('normalScale') }}<output>{{ value('normalScale').toFixed(2) }}</output></span><input type="range" min="0" max="4" step="0.05" :disabled="pbrLocked" :value="value('normalScale')" @input="number($event,'normalScale')" @change="number($event,'normalScale',true)"/></label>
      <button v-if="binding" class="reset" @click="emit('reset')">{{ t('resetMaterial') }}</button>
    </div>
  </details>
</template>

<style scoped>
.material-editor{border-bottom:1px solid var(--e-border);font-size:11px;color:var(--e-text)}summary{cursor:pointer;padding:12px 8px;overflow-wrap:anywhere}.swatch{display:inline-block;width:16px;height:16px;border:1px solid #ffffff25;border-radius:3px;vertical-align:middle;margin-right:8px}.material-fields{padding:0 10px 12px;display:grid;gap:12px}.material-fields p{margin:0;color:var(--e-muted);font-size:10px;line-height:1.5}.material-fields label{display:block}.material-fields label>span{display:flex;justify-content:space-between;gap:8px}.material-fields input[type=range]{width:100%;accent-color:#c8cdd7;margin-top:7px}.material-fields .inline,.texture{display:flex;align-items:center;justify-content:space-between;gap:8px;flex-wrap:wrap}.texture>span{min-width:0;flex:1 1 100%}.texture small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:120px;color:var(--e-muted);margin-top:3px}.material-fields select,.material-fields button{font:inherit;background:#191b1f;color:inherit;border:1px solid var(--e-border);border-radius:4px;padding:5px;cursor:pointer}.material-fields input[type=color]{width:36px;height:24px;padding:0;border:0;background:none}.material-fields output{font-variant-numeric:tabular-nums;color:var(--e-muted)}.material-fields input:disabled{opacity:.4;cursor:not-allowed}.reset{width:100%}
</style>
