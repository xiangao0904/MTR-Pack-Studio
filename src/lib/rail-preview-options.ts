export const builtInRailInterval = 0.6

export const builtInRails = [
  { id: '', model: 'rail.obj', texture: 'rail_base_color.png', label: 'builtInRail' },
  { id: 'builtin:sleeper', model: 'rail_sleeper.obj', texture: 'sleepter_base_color.png', label: 'builtInSleeperRail' },
  { id: 'builtin:siding', model: 'rail_siding.obj', texture: 'siding_base_color.png', label: 'builtInSidingRail' },
] as const

export type BuiltInRailId = (typeof builtInRails)[number]['id']
export function isBuiltInRailId(value: string): value is BuiltInRailId {
  return builtInRails.some(rail => rail.id === value)
}
