<script setup lang="ts">
import { t } from '../i18n'
import { defaultViewportSettings, renderModes, type PreviewRenderMode, type ViewportSettings } from '../lib/viewport-settings'

const props = defineProps<{
  mode: PreviewRenderMode
  settings: ViewportSettings
  showGrid: boolean
  wireframe: boolean
  showRails: boolean
  railModelId: string
  railOptions: { id: string; name: string }[]
}>()
const emit = defineEmits<{
  'update:mode': [value: PreviewRenderMode]
  'update:settings': [value: ViewportSettings]
  'update:showGrid': [value: boolean]
  'update:wireframe': [value: boolean]
  'update:showRails': [value: boolean]
  'update:railModelId': [value: string]
}>()

const sliders = [
  { key: 'lightAzimuth', label: 'lightAzimuth', min: 0, max: 360, step: 1, unit: '°' },
  { key: 'lightElevation', label: 'lightElevation', min: 10, max: 85, step: 1, unit: '°' },
  { key: 'lightIntensity', label: 'lightIntensity', min: 0, max: 8, step: .1, unit: '' },
  { key: 'environmentIntensity', label: 'environmentIntensity', min: 0, max: 3, step: .1, unit: '' },
  { key: 'lightSize', label: 'lightSize', min: .1, max: 30, step: .1, unit: '°' },
  { key: 'indirectIntensity', label: 'indirectIntensity', min: 0, max: 2, step: .1, unit: '' },
] as const
function update<K extends keyof ViewportSettings>(key: K, value: ViewportSettings[K]) {
  emit('update:settings', { ...props.settings, [key]: value })
}
function reset() {
  emit('update:settings', defaultViewportSettings(props.mode))
  emit('update:showGrid', props.mode === 'studio')
  emit('update:wireframe', false)
}
</script>

<template>
  <div class="scene-settings">
    <div class="scene-section">
      <h3>{{ t('sceneDisplay') }}</h3>
      <label class="scene-select"><span>{{ t('renderMode') }}</span><select :value="mode" @change="emit('update:mode', ($event.target as HTMLSelectElement).value as PreviewRenderMode)"><option v-for="item in renderModes" :key="item" :value="item">{{ t(item === 'studio' ? 'renderStudio' : item === 'material' ? 'renderMaterial' : 'renderMinecraft') }}</option></select></label>
      <label class="scene-check"><span>{{ t('grid') }}</span><input type="checkbox" :checked="showGrid" @change="emit('update:showGrid', ($event.target as HTMLInputElement).checked)" /></label>
      <label class="scene-check"><span>{{ t('wireframe') }}</span><input type="checkbox" :checked="wireframe" @change="emit('update:wireframe', ($event.target as HTMLInputElement).checked)" /></label>
      <label class="scene-check"><span>{{ t('showRails') }}</span><input type="checkbox" :checked="showRails" @change="emit('update:showRails', ($event.target as HTMLInputElement).checked)" /></label>
      <label class="scene-select"><span>{{ t('previewRailModel') }}</span><select :value="railModelId" @change="emit('update:railModelId', ($event.target as HTMLSelectElement).value)"><option value="">{{ t('builtInRail') }}</option><option v-for="rail in railOptions" :key="rail.id" :value="rail.id">{{ rail.name }}</option></select></label>
      <label v-if="mode === 'material'" class="scene-check"><span>{{ t('ground') }}</span><input type="checkbox" :checked="settings.ground" @change="update('ground', ($event.target as HTMLInputElement).checked)" /></label>
    </div>
    <div class="scene-section">
      <h3>{{ t('sceneLighting') }}</h3>
      <label v-for="item in sliders" :key="item.key" class="scene-slider"><span>{{ t(item.label) }}<output>{{ settings[item.key] }}{{ item.unit }}</output></span><input type="range" :min="item.min" :max="item.max" :step="item.step" :value="settings[item.key]" @input="update(item.key, Number(($event.target as HTMLInputElement).value))" /></label>
      <template v-if="mode === 'minecraft'"><label class="scene-slider"><span>{{ t('cloudCover') }}<output>{{ Math.round(settings.cloudCover * 100) }}%</output></span><input type="range" min="0" max="1" step="0.05" :value="settings.cloudCover" @input="update('cloudCover', Number(($event.target as HTMLInputElement).value))" /></label><label class="scene-slider"><span>{{ t('skyHaze') }}<output>{{ Math.round(settings.skyHaze * 100) }}%</output></span><input type="range" min="0" max="1" step="0.05" :value="settings.skyHaze" @input="update('skyHaze', Number(($event.target as HTMLInputElement).value))" /></label></template>
    </div>
    <div class="scene-section">
      <h3>{{ t('sceneQuality') }}</h3>
      <label class="scene-check"><span>{{ t('ambientOcclusion') }}</span><input type="checkbox" :checked="settings.ambientOcclusion" @change="update('ambientOcclusion', ($event.target as HTMLInputElement).checked)" /></label>
      <label class="scene-check"><span>{{ t('pixelTextures') }}</span><input type="checkbox" :checked="settings.pixelTextures" @change="update('pixelTextures', ($event.target as HTMLInputElement).checked)" /></label>
      <label class="scene-select"><span>{{ t('shadowQuality') }}</span><select :value="settings.shadowQuality" @change="update('shadowQuality', ($event.target as HTMLSelectElement).value as ViewportSettings['shadowQuality'])"><option value="standard">{{ t('qualityStandard') }}</option><option value="high">{{ t('qualityHigh') }}</option></select></label>
    </div>
    <button class="scene-reset" type="button" @click="reset">{{ t('resetViewportSettings') }}</button>
  </div>
</template>

<style scoped>
.scene-settings{padding:0 13px 15px;color:var(--e-muted);font-size:12px}.scene-section{padding:14px 0;border-bottom:1px solid var(--e-border)}.scene-section h3{margin:0 0 12px;color:var(--e-text);font-size:12px;font-weight:600}.scene-check{display:flex;align-items:center;justify-content:space-between;gap:10px;min-height:30px;cursor:pointer}.scene-check input{width:15px;height:15px;accent-color:#c2c7d0}.scene-select{display:grid;gap:7px;margin:10px 0}.scene-select select{min-width:0;width:100%;height:31px;padding:5px 8px;color:var(--e-text);background:var(--e-bg);border:1px solid var(--e-border);border-radius:4px}.scene-slider{display:block;margin:12px 0}.scene-slider span{display:flex;justify-content:space-between;gap:8px}.scene-slider output{color:var(--e-text);font-variant-numeric:tabular-nums}.scene-slider input{width:100%;margin:6px 0 0;accent-color:#b7c0ca}.scene-reset{width:100%;margin-top:14px;padding:8px;border:1px solid var(--e-border);border-radius:4px;background:var(--e-bg);color:var(--e-text);cursor:pointer}.scene-reset:hover{background:var(--e-hover)}
</style>
