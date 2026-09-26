<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Box, ChevronDown, ChevronRight, Eye, EyeOff, FileBox, Folder } from '@lucide/vue'
import { t } from '../i18n'
import type { AssetDefinition, ModelLayer } from '../lib/projects'
import { buildPartTree, flattenPartTree } from '../lib/model-tree'

const props = defineProps<{ storageKey: string; name: string; rootLabel?: string; layers: ModelLayer[]; assets: Record<string, AssetDefinition>; query: string; selectedLayer: string; selectedPart: string }>()
const emit = defineEmits<{ select: [layerId: string, partId?: string]; visibility: [layerId: string | null, partIds?: string[]] }>()
const collapsed = ref(new Set<string>())
watch(() => props.storageKey, key => { try { collapsed.value = new Set(JSON.parse(localStorage.getItem(key) || '[]')) } catch { collapsed.value = new Set() } }, { immediate: true })
function toggle(id: string) { const next = new Set(collapsed.value); next.has(id) ? next.delete(id) : next.add(id); collapsed.value = next; localStorage.setItem(props.storageKey, JSON.stringify([...next])) }
const rootOpen = computed(() => !!props.query.trim() || !collapsed.value.has('root'))
function layerOpen(layer: ModelLayer) { return !!props.query.trim() || !collapsed.value.has(layer.id) }
function rows(layer: ModelLayer) {
  const nodes = buildPartTree(props.assets[layer.assetId]?.parts || [])
  const local = new Set([...collapsed.value].filter(key => key.startsWith(`${layer.id}:`)).map(key => key.slice(layer.id.length + 1)))
  return flattenPartTree(nodes, local, layer.name.toLowerCase().includes(props.query.toLowerCase()) ? '' : props.query)
}
const visibleLayers = computed(() => props.layers.filter(layer => !props.query.trim() || layer.name.toLowerCase().includes(props.query.toLowerCase()) || rows(layer).length))
function shown(layer: ModelLayer, ids?: string[]) { return layer.visible && (!ids || ids.some(id => !(layer.hiddenParts || []).includes(id))) }
function mixed(layer: ModelLayer, ids: string[]) { const hidden = ids.filter(id => (layer.hiddenParts || []).includes(id)).length; return layer.visible && hidden > 0 && hidden < ids.length }
function revealSelection() {
  if (!props.selectedLayer) return
  const next = new Set(collapsed.value); next.delete('root'); next.delete(props.selectedLayer)
  const layer = props.layers.find(layer => layer.id === props.selectedLayer)
  const visit = (nodes: ReturnType<typeof buildPartTree>) => { for (const node of nodes) if (node.children.length && node.partIds.includes(props.selectedPart)) { next.delete(`${props.selectedLayer}:${node.id}`); visit(node.children) } }
  if (layer && props.selectedPart) visit(buildPartTree(props.assets[layer.assetId]?.parts || []))
  collapsed.value = next
}
watch(() => [props.selectedLayer, props.selectedPart], revealSelection)
</script>
<template>
  <div class="model-hierarchy" role="tree" :aria-label="t('modelTree')">
    <div class="tree-head"><span>{{ t('fieldName') }}</span><span>{{ t('type') }}</span><span>{{ t('visible') }}</span></div>
    <div class="tree-row" role="treeitem" :aria-expanded="rootOpen">
      <span class="node-name"><button class="disclosure" :aria-label="`${rootOpen ? t('collapse') : t('expand')} ${name}`" @click="toggle('root')"><component :is="rootOpen ? ChevronDown : ChevronRight" :size="14" /></button><Box :size="15" /><strong>{{ name }}</strong></span>
      <small>{{ rootLabel || t('carriage') }}</small><button class="eye" :disabled="!layers.length" :aria-label="`${t('toggleVisibility')} ${name}`" :aria-pressed="layers.some(layer => layer.visible)" @click="emit('visibility', null)"><component :is="layers.some(layer => layer.visible) ? Eye : EyeOff" :size="15" /></button>
    </div>
    <Transition name="tree-expand"><div v-if="rootOpen" role="group" class="tree-branch">
      <template v-for="layer in visibleLayers" :key="layer.id">
        <div :class="['tree-row', { active: selectedLayer === layer.id && !selectedPart, muted: !layer.visible }]" role="treeitem" :aria-expanded="layerOpen(layer)">
          <span class="node-name" style="padding-left:16px"><button class="disclosure" :aria-label="`${layerOpen(layer) ? t('collapse') : t('expand')} ${layer.name}`" @click="toggle(layer.id)"><component :is="layerOpen(layer) ? ChevronDown : ChevronRight" :size="14" /></button><button class="node-select" @click="emit('select',layer.id)"><FileBox :size="15" />{{ layer.name }}</button></span>
          <small>{{ t('modelLayer') }}</small><button class="eye" :aria-label="`${t('toggleVisibility')} ${layer.name}`" :aria-pressed="layer.visible" @click="emit('visibility',layer.id)"><component :is="layer.visible ? Eye : EyeOff" :size="15" /></button>
        </div>
        <Transition name="tree-expand"><div v-if="layerOpen(layer)" role="group" class="tree-branch"><TransitionGroup name="tree-nodes" tag="div" :css="!query.trim()">
          <div v-for="node in rows(layer)" :key="node.id" :class="['tree-row',{active:selectedLayer===layer.id && selectedPart===node.id,muted:!shown(layer,node.partIds)}]" role="treeitem" :aria-expanded="node.children.length ? (!!query.trim() || !collapsed.has(`${layer.id}:${node.id}`)) : undefined">
            <span class="node-name" :style="{paddingLeft:`${32+node.depth*16}px`}"><button v-if="node.children.length" class="disclosure" :aria-label="`${collapsed.has(`${layer.id}:${node.id}`) ? t('expand') : t('collapse')} ${node.name}`" @click="toggle(`${layer.id}:${node.id}`)"><component :is="collapsed.has(`${layer.id}:${node.id}`) && !query ? ChevronRight : ChevronDown" :size="14" /></button><span v-else class="leaf-spacer" /><button class="node-select" @click="node.children.length ? toggle(`${layer.id}:${node.id}`) : emit('select',layer.id,node.id)"><component :is="node.children.length ? Folder : Box" :size="14" />{{ node.name }}</button></span>
            <small>{{ node.children.length ? t('group') : t('mesh') }}</small><button :class="['eye',{mixed:mixed(layer,node.partIds)}]" :aria-label="`${t('toggleVisibility')} ${node.name}`" :aria-pressed="shown(layer,node.partIds)" :title="t('visibilityHint')" @click="emit('visibility',layer.id,node.partIds)"><component :is="shown(layer,node.partIds) ? Eye : EyeOff" :size="15" /></button>
          </div>
        </TransitionGroup></div></Transition>
      </template>
      <p v-if="!visibleLayers.length" class="tree-empty">{{ query ? t('noMatches') : t('noModelAssigned') }}</p>
    </div></Transition>
  </div>
</template>
<style scoped>
.model-hierarchy{min-height:0;overflow:auto;padding:0 10px 8px;font-family:var(--studio-font,'Segoe UI Variable','Segoe UI',sans-serif)}
.tree-head,.tree-row{display:grid;grid-template-columns:minmax(0,1fr) 82px 44px;align-items:center;min-height:27px;font-size:12px;color:var(--studio-muted,#a4a5ad)}
.tree-head{position:sticky;top:0;z-index:1;min-height:25px;border-bottom:1px solid var(--studio-border,#303136);background:var(--studio-surface,#1b1c1f);color:var(--studio-quiet,#73757e);font-size:10px}.tree-head>span:last-child{text-align:center}
.tree-row{border-radius:3px}.tree-row:hover{background:var(--studio-hover,#28292e)}.tree-row.active{background:var(--studio-selected,#33343a);color:var(--studio-text,#e8e8ec);box-shadow:inset 2px 0 0 #aaacb5}
.tree-branch{position:relative;min-width:0}.tree-expand-enter-active,.tree-expand-leave-active{transition:opacity var(--motion-panel,180ms) ease,transform var(--motion-panel,180ms) var(--motion-ease,ease);overflow:hidden}.tree-expand-enter-from,.tree-expand-leave-to{opacity:0;transform:translateY(-4px)}
.tree-nodes-enter-active,.tree-nodes-leave-active{transition:opacity 150ms ease,transform 150ms var(--motion-ease,ease)}.tree-nodes-enter-from,.tree-nodes-leave-to{opacity:0;transform:translateY(-3px)}.tree-nodes-leave-active{position:absolute;pointer-events:none}
.node-name{display:flex;align-items:center;min-width:0;gap:5px}.node-name>svg{width:14px;height:14px;flex:none}.node-name strong{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:500}
.node-select{display:flex;align-items:center;gap:6px;flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;text-align:left}.node-select svg{flex:none;width:14px;height:14px;stroke-width:1.5}
button{padding:3px;border:0;background:none;color:inherit;font:inherit;cursor:pointer}.disclosure,.leaf-spacer{display:inline-flex;align-items:center;justify-content:center;width:18px;height:23px;flex:none;border-radius:3px}
.disclosure svg{transition:transform var(--motion-feedback,120ms) ease}
.eye{display:grid;place-items:center;justify-self:center;width:27px;height:25px;border-radius:3px;color:var(--studio-muted,#a4a5ad)}.eye svg{width:14px;height:14px;stroke-width:1.5}.eye:hover,.disclosure:hover{background:#ffffff0c;color:var(--studio-text,#e8e8ec)}.eye:disabled{opacity:.3;cursor:default}.eye.mixed{opacity:.55}.muted .node-name,.muted small{opacity:.4}
small{font-size:10px;color:var(--studio-quiet,#73757e)}.tree-empty{padding:12px;color:var(--studio-quiet,#73757e);font-size:12px}button:focus-visible{outline:1px solid #b8bac3;outline-offset:-1px}
@media(prefers-reduced-motion:reduce){.tree-expand-enter-active,.tree-expand-leave-active,.tree-nodes-enter-active,.tree-nodes-leave-active,.disclosure svg{transition:none!important}}
</style>
