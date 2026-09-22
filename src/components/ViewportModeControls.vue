<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, useId } from 'vue'
import { Settings2 } from '@lucide/vue'
import { t } from '../i18n'
import { defaultViewportSettings, renderModes, type PreviewRenderMode, type ViewportSettings } from '../lib/viewport-settings'

const props = defineProps<{ mode: PreviewRenderMode; settings: ViewportSettings; showGrid: boolean; wireframe: boolean }>()
const emit = defineEmits<{
  'update:mode': [mode: PreviewRenderMode]
  'update:settings': [settings: ViewportSettings]
  'update:showGrid': [value: boolean]
  'update:wireframe': [value: boolean]
}>()
const host = ref<HTMLElement>()
const settingsButton = ref<HTMLButtonElement>()
const panel = ref<HTMLElement>()
const open = ref(false)
const panelPosition = ref({ top: '0px', left: '0px', maxHeight: '400px' })
const id = useId().replace(/:/g, '')
const label = (mode: PreviewRenderMode) => mode === 'studio' ? t('renderStudio') : mode === 'material' ? t('renderMaterial') : t('renderMinecraft')
const hint = (mode: PreviewRenderMode) => mode === 'studio' ? t('studioModeHint') : mode === 'material' ? t('materialModeHint') : t('minecraftModeHint')

function update<K extends keyof ViewportSettings>(key: K, value: ViewportSettings[K]) {
  emit('update:settings', { ...props.settings, [key]: value })
}
function slider(event: Event, key: 'lightAzimuth' | 'lightElevation' | 'lightIntensity' | 'environmentIntensity') {
  update(key, Number((event.target as HTMLInputElement).value))
}
function reset() {
  emit('update:settings', defaultViewportSettings(props.mode))
  emit('update:showGrid', props.mode === 'studio')
  emit('update:wireframe', false)
}
function modeKey(event: KeyboardEvent) {
  let index = renderModes.indexOf(props.mode)
  if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') index = (index + renderModes.length - 1) % renderModes.length
  else if (event.key === 'ArrowRight' || event.key === 'ArrowDown') index = (index + 1) % renderModes.length
  else if (event.key === 'Home') index = 0
  else if (event.key === 'End') index = renderModes.length - 1
  else return
  event.preventDefault()
  emit('update:mode', renderModes[index]!)
  host.value?.querySelectorAll<HTMLButtonElement>('[role="radio"]')[index]?.focus()
}
async function toggleSettings() {
  open.value = !open.value
  if (open.value) {
    await nextTick()
    positionPanel()
    panel.value?.querySelector<HTMLInputElement>('input')?.focus()
  }
}
function positionPanel() {
  if (!open.value || !settingsButton.value) return
  const button = settingsButton.value.getBoundingClientRect()
  const margin = 8
  const width = Math.min(276, window.innerWidth - margin * 2)
  const top = Math.max(margin, Math.min(button.bottom + margin, window.innerHeight - 150))
  panelPosition.value = {
    top: `${top}px`,
    left: `${Math.max(margin, Math.min(button.right - width, window.innerWidth - width - margin))}px`,
    maxHeight: `${Math.max(80, window.innerHeight - top - margin)}px`,
  }
}
function contains(target: EventTarget | null) {
  return target instanceof Node && (host.value?.contains(target) || panel.value?.contains(target))
}
function outside(event: PointerEvent) {
  if (open.value && !contains(event.target)) open.value = false
}
function escape(event: KeyboardEvent) {
  if (open.value && event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    open.value = false
    settingsButton.value?.focus()
  }
}
function focusOut(event: FocusEvent) {
  if (event.relatedTarget && !contains(event.relatedTarget)) open.value = false
}
onMounted(() => {
  document.addEventListener('pointerdown', outside)
  document.addEventListener('keydown', escape, true)
  window.addEventListener('resize', positionPanel)
  document.addEventListener('scroll', positionPanel, true)
})
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', outside)
  document.removeEventListener('keydown', escape, true)
  window.removeEventListener('resize', positionPanel)
  document.removeEventListener('scroll', positionPanel, true)
})
</script>

<template>
  <div ref="host" class="viewport-modes" @focusout="focusOut">
    <div class="mode-group" role="radiogroup" :aria-label="t('renderMode')" @keydown="modeKey">
      <button v-for="item in renderModes" :key="item" type="button" class="mode-button" :class="{ active: mode === item }" role="radio" :aria-checked="mode === item" :aria-label="label(item)" :title="`${label(item)} — ${hint(item)}`" :tabindex="mode === item ? 0 : -1" @click="emit('update:mode', item)">
        <svg v-if="item === 'studio'" viewBox="0 0 32 32" aria-hidden="true">
          <defs><radialGradient :id="`${id}-studio`" cx="30%" cy="22%" r="82%"><stop offset="0" stop-color="#eef2f5"/><stop offset=".36" stop-color="#bdc7cf"/><stop offset=".73" stop-color="#727f8a"/><stop offset="1" stop-color="#3b4854"/></radialGradient></defs>
          <ellipse cx="16" cy="27" rx="10" ry="2" fill="#080e15" opacity=".6"/>
          <circle cx="16" cy="15" r="12" :fill="`url(#${id}-studio)`" stroke="#cbd7e1" stroke-opacity=".5" stroke-width=".8"/>
          <path d="M7 19c3 6 10 8 16 3" fill="none" stroke="#aab9c5" stroke-opacity=".22" stroke-width="1.1"/>
        </svg>
        <svg v-else-if="item === 'material'" viewBox="0 0 32 32" aria-hidden="true">
          <defs><radialGradient :id="`${id}-material`" cx="33%" cy="23%" r="86%"><stop offset="0" stop-color="#fbf6e9"/><stop offset=".26" stop-color="#d9c5a3"/><stop offset=".47" stop-color="#a19582"/><stop offset=".5" stop-color="#566875"/><stop offset=".7" stop-color="#293b4b"/><stop offset="1" stop-color="#9fb8cd"/></radialGradient><clipPath :id="`${id}-clip`"><circle cx="16" cy="15" r="12"/></clipPath></defs>
          <ellipse cx="16" cy="27" rx="10" ry="2" fill="#080e15" opacity=".6"/>
          <circle cx="16" cy="15" r="12" :fill="`url(#${id}-material)`" stroke="#d2dce4" stroke-opacity=".6" stroke-width=".8"/>
          <g :clip-path="`url(#${id}-clip)`"><path d="M5 9c5-5 11-7 16-5l-3 5c-4-1-8 1-11 4z" fill="#fff" opacity=".85"/><path d="M20 6c3 2 5 5 5 9l-2 1c0-4-1-6-3-8z" fill="#fff" opacity=".5"/><path d="M7 20c6 3 13 3 19-1" fill="none" stroke="#b1cfdf" stroke-width="1.5" opacity=".65"/></g>
        </svg>
        <svg v-else viewBox="0 0 32 32" aria-hidden="true" shape-rendering="crispEdges">
          <path d="m16 2 13 7v15l-13 7-13-7V9z" fill="#5d4230"/>
          <path d="m3 9 13 7v15L3 24z" fill="#8b6441"/><path d="m16 16 13-7v15l-13 7z" fill="#63472f"/>
          <path d="m16 2 13 7-13 7L3 9z" fill="#86ad59"/><path d="m16 2 5 3-5 3-5-3zM7 7l5 3-4 2-5-3z" fill="#a1c86a"/><path d="m20 7 5 3-5 3-5-3z" fill="#70994a"/>
          <path d="m3 9 13 7v5l-4-2v-2l-4-2v2l-5-3z" fill="#669049"/><path d="m16 16 13-7v5l-4 2v-2l-4 2v3l-5 3z" fill="#4d7439"/>
          <path d="m5 18 4 2v3l-4-2zm6 6 3 1v3l-3-1z" fill="#ac8053"/><path d="m19 24 3-2v3l-3 2zm6-5 3-1v3l-3 1z" fill="#836143"/>
          <path d="m16 2 13 7v15l-13 7-13-7V9z" fill="none" stroke="#c2d3be" stroke-opacity=".35" stroke-width=".7"/>
        </svg>
        <span class="mode-tooltip" role="tooltip"><strong>{{ label(item) }}</strong>{{ hint(item) }}</span>
      </button>
    </div>
    <button ref="settingsButton" class="settings-button" :class="{ active: open }" type="button" :title="t('viewportSettings')" :aria-label="t('viewportSettings')" aria-haspopup="dialog" :aria-expanded="open" :aria-controls="`${id}-settings`" @click="toggleSettings"><Settings2 :size="17"/></button>
    <Teleport to="body">
    <section v-if="open" :id="`${id}-settings`" ref="panel" class="settings-panel" :style="panelPosition" role="dialog" :aria-label="t('viewportSettings')" @focusout="focusOut">
      <header>{{ label(mode) }}<span>{{ t('viewportSettings') }}</span></header>
      <div class="toggle-row"><label><input type="checkbox" :checked="showGrid" @change="emit('update:showGrid', ($event.target as HTMLInputElement).checked)">{{ t('grid') }}</label><label><input type="checkbox" :checked="wireframe" @change="emit('update:wireframe', ($event.target as HTMLInputElement).checked)">{{ t('wireframe') }}</label></div>
      <template v-if="mode !== 'studio'">
        <label class="quality-row"><span>{{ t('shadowQuality') }}</span><select :value="settings.shadowQuality" @change="update('shadowQuality', ($event.target as HTMLSelectElement).value as ViewportSettings['shadowQuality'])"><option value="standard">{{ t('qualityStandard') }}</option><option value="high">{{ t('qualityHigh') }}</option></select></label>
        <label class="slider-row"><span>{{ t('lightAzimuth') }}<output>{{ settings.lightAzimuth }}°</output></span><input type="range" min="0" max="360" step="1" :aria-label="t('lightAzimuth')" :value="settings.lightAzimuth" @input="slider($event, 'lightAzimuth')"></label>
        <label class="slider-row"><span>{{ t('lightElevation') }}<output>{{ settings.lightElevation }}°</output></span><input type="range" min="10" max="85" step="1" :aria-label="t('lightElevation')" :value="settings.lightElevation" @input="slider($event, 'lightElevation')"></label>
        <label class="slider-row"><span>{{ t('lightIntensity') }}<output>{{ settings.lightIntensity.toFixed(1) }}</output></span><input type="range" min="0" max="8" step="0.1" :aria-label="t('lightIntensity')" :value="settings.lightIntensity" @input="slider($event, 'lightIntensity')"></label>
        <label class="slider-row"><span>{{ t('environmentIntensity') }}<output>{{ settings.environmentIntensity.toFixed(1) }}</output></span><input type="range" min="0" max="3" step="0.1" :aria-label="t('environmentIntensity')" :value="settings.environmentIntensity" @input="slider($event, 'environmentIntensity')"></label>
        <label v-if="mode === 'material'" class="ground-toggle"><input type="checkbox" :checked="settings.ground" @change="update('ground', ($event.target as HTMLInputElement).checked)">{{ t('ground') }}</label>
      </template>
      <button type="button" class="reset-button" @click="reset">{{ t('resetViewportSettings') }}</button>
    </section>
    </Teleport>
  </div>
</template>

<style scoped>
.viewport-modes{position:relative;display:inline-flex;align-items:center;gap:5px;flex-shrink:0;color:#c7d4e1}
.mode-group{display:flex;align-items:center;gap:2px;padding:3px;border:1px solid #364651;border-radius:7px;background:#141d23}
.mode-button,.settings-button{display:flex;align-items:center;justify-content:center;border:1px solid transparent;border-radius:4px;background:transparent;color:inherit;cursor:pointer;padding:3px;width:34px;height:34px;position:relative}
.mode-button svg{width:28px;height:28px;flex:none}.mode-button:hover,.settings-button:hover{background:#2a3b48}.mode-button.active,.settings-button.active{background:#294257;border-color:#89bfe8;box-shadow:inset 0 0 0 1px #aad7ff12}
.mode-button:focus-visible,.settings-button:focus-visible,.reset-button:focus-visible{outline:2px solid #acdaff;outline-offset:2px}.settings-button{width:29px;height:34px}
.mode-tooltip{display:none;position:absolute;top:calc(100% + 12px);left:50%;transform:translateX(-50%);width:190px;padding:10px 12px;white-space:normal;text-align:left;font-size:11px;line-height:1.5;font-weight:400;background:#17232d;border:1px solid #435767;border-radius:6px;box-shadow:0 6px 24px #0007;pointer-events:none;z-index:65;color:#aebfcf}.mode-tooltip strong{display:block;color:#e0eaf3;font-size:12px;margin-bottom:3px}.mode-button:hover .mode-tooltip,.mode-button:focus-visible .mode-tooltip{display:block}
.settings-panel{position:fixed;z-index:1000;width:276px;max-width:calc(100vw - 16px);overflow-y:auto;overscroll-behavior:contain;padding:15px;background:#1a252e;border:1px solid #405362;border-radius:8px;box-shadow:0 12px 36px #0008;font-size:12px;box-sizing:border-box;color:#c7d4e1}
.settings-panel header{display:flex;flex-direction:column;gap:3px;font-size:13px;font-weight:600;color:#e3edf5;padding-bottom:12px;border-bottom:1px solid #35454f}.settings-panel header span{font-size:11px;font-weight:400;color:#92a7b8}.toggle-row{display:flex;justify-content:space-between;margin:14px 0;gap:12px}.toggle-row label,.ground-toggle{display:flex;align-items:center;gap:7px;cursor:pointer}.settings-panel input{accent-color:#9dccf2}.settings-panel input[type=checkbox]{margin:0;width:14px;height:14px}
.quality-row{display:flex;align-items:center;justify-content:space-between;margin:16px 0 13px;gap:10px}.quality-row select{background:#111d26;border:1px solid #415666;border-radius:4px;color:#d8e7f3;padding:5px 7px;font:inherit;max-width:120px}.slider-row{display:block;margin:13px 0}.slider-row>span{display:flex;justify-content:space-between;align-items:center;gap:8px}.slider-row output{font-variant-numeric:tabular-nums;color:#a5c7e2;font-size:11px}.slider-row input{width:100%;height:15px;margin:6px 0 0;cursor:pointer}.ground-toggle{margin:14px 0}.reset-button{width:100%;margin-top:6px;padding:8px;border:1px solid #425463;border-radius:4px;background:#243440;color:#d4e2ee;cursor:pointer;font:inherit}.reset-button:hover{background:#304858}
</style>
