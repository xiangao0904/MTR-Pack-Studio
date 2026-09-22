export type PreviewRenderMode = 'studio' | 'material' | 'minecraft'
export interface ViewportSettings {
  shadowQuality: 'standard' | 'high'
  lightAzimuth: number
  lightElevation: number
  lightSize: number
  indirectIntensity: number
  lightIntensity: number
  environmentIntensity: number
  ambientOcclusion: boolean
  pixelTextures: boolean
  ground: boolean
}
export const renderModes: PreviewRenderMode[] = ['studio', 'material', 'minecraft']
export function defaultViewportSettings(mode: PreviewRenderMode): ViewportSettings {
  return { lightSize: mode === 'minecraft' ? 2 : 12, indirectIntensity: 1, ambientOcclusion: true, pixelTextures: false, shadowQuality: 'standard', lightAzimuth: 135, lightElevation: mode === 'minecraft' ? 38 : 50, lightIntensity: 3, environmentIntensity: 0.8, ground: true }
}
export function normalizeViewportSettings(mode: PreviewRenderMode, value: unknown): ViewportSettings {
  const defaults = defaultViewportSettings(mode)
  const input = value && typeof value === 'object' ? value as Partial<ViewportSettings> : {}
  const number = (key: keyof ViewportSettings, min: number, max: number) => typeof input[key] === 'number' && Number.isFinite(input[key]) ? Math.min(max, Math.max(min, input[key] as number)) : defaults[key] as number
  return { lightSize: number('lightSize', .1, 30), indirectIntensity: number('indirectIntensity', 0, 2), ambientOcclusion: typeof input.ambientOcclusion === 'boolean' ? input.ambientOcclusion : true, pixelTextures: input.pixelTextures === true, shadowQuality: input.shadowQuality === 'high' ? 'high' : 'standard', lightAzimuth: number('lightAzimuth', 0, 360), lightElevation: number('lightElevation', 10, 85), lightIntensity: number('lightIntensity', 0, 8), environmentIntensity: number('environmentIntensity', 0, 3), ground: typeof input.ground === 'boolean' ? input.ground : defaults.ground }
}
export function loadViewportPreferences() {
  let saved: Record<string, unknown> = {}
  try { saved = JSON.parse(localStorage.getItem('mtr-pack-studio:viewport-settings') || '{}') || {} } catch { /* Restore defaults if preferences are damaged. */ }
  return Object.fromEntries(renderModes.map(mode => [mode, normalizeViewportSettings(mode, saved[mode])])) as Record<PreviewRenderMode, ViewportSettings>
}
