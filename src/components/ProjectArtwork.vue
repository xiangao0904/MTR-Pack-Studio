<script setup lang="ts">
import { ref, watch, onBeforeUnmount } from 'vue'
import { getProjectCover } from '../lib/projects'
const props = defineProps<{ path?: string; revision?: number }>()
const cover = ref('')
let sequence = 0
watch(() => [props.path, props.revision], async () => {
  const request = ++sequence
  try {
    const bytes = props.path ? await getProjectCover(props.path) : null
    if (request !== sequence) return
    if (cover.value) URL.revokeObjectURL(cover.value)
    cover.value = bytes?.byteLength ? URL.createObjectURL(new Blob([bytes], { type: 'image/png' })) : ''
  } catch { if (request === sequence) { if (cover.value) URL.revokeObjectURL(cover.value); cover.value = '' } }
}, { immediate: true })
onBeforeUnmount(() => { sequence++; if (cover.value) URL.revokeObjectURL(cover.value) })
</script>
<template>
  <span class="project-artwork">
    <img v-if="cover" :src="cover" alt="" />
    <img v-else class="default-artwork" :src="'/images/train-placeholder.svg'" alt="" />
  </span>
</template>
<style scoped>
.project-artwork{display:grid;place-items:center;width:100%;height:100%;overflow:hidden;border-radius:inherit;background:radial-gradient(ellipse at 15% 0%,#526b7c,#25333f 65%,#1a242d)}svg{width:100%;height:100%}img{width:100%;height:100%;object-fit:cover}
</style>
