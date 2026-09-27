<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ label: string; current?: number; total?: number }>()
const percent = computed(() => props.total && props.total > 0 && props.current !== undefined
  ? Math.round(Math.min(1, Math.max(0, props.current / props.total)) * 100)
  : undefined)
</script>

<template>
  <div class="loading-progress" role="progressbar" :aria-label="label" :aria-valuenow="percent" :aria-valuemin="percent === undefined ? undefined : 0" :aria-valuemax="percent === undefined ? undefined : 100">
    <span class="loading-label">{{ label }}</span><span v-if="percent !== undefined" class="loading-percent">{{ percent }}%</span>
    <div class="loading-track"><div :class="['loading-fill', { indeterminate: percent === undefined }]" :style="percent === undefined ? undefined : { width: `${percent}%` }" /></div>
  </div>
</template>

<style scoped>
.loading-progress{display:grid;grid-template-columns:1fr auto;gap:8px 12px;width:min(320px,calc(100% - 32px));color:var(--studio-muted,#a4a5ad);font-size:12px}
.loading-label{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.loading-percent{font-variant-numeric:tabular-nums}
.loading-track{grid-column:1/-1;height:4px;overflow:hidden;border-radius:4px;background:#ffffff22}
.loading-fill{height:100%;border-radius:inherit;background:#9dbce1;transition:width .18s ease-out}
.loading-fill.indeterminate{width:35%;animation:loading-slide 1.35s ease-in-out infinite}
@keyframes loading-slide{from{transform:translateX(-110%)}to{transform:translateX(310%)}}
@media(prefers-reduced-motion:reduce){.loading-fill.indeterminate{animation:none;width:100%;opacity:.6}}
</style>
