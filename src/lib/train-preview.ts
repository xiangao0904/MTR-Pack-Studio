import type { CarPlacementRule, TrainDefinition } from './projects'

/** MTR filters use one-based positions; negative positions count from the tail. */
export function expressionStrength(expression: string, position: number, count: number): number {
  return Math.max(0, ...expression.split(',').map(value => value.trim()).filter(Boolean).map(token => {
    const periodic = /^%(\d+)(\+-?\d+|-\d+)?$/.exec(token)
    if (periodic) { const step = Number(periodic[1]); return step > 0 && (position + Number(periodic[2]?.replace(/^\+/, '') || 0)) % step === 0 ? 2 : 0 }
    if (!/^-?[1-9]\d*$/.test(token)) return 0
    const value = Number(token)
    return position === (value < 0 ? count + value + 1 : value) ? 3 : 0
  }))
}

export function matchesExpression(expression: string, position: number, count: number): boolean { return expressionStrength(expression, position, count) > 0 }

export function matchesPlacement(rule: CarPlacementRule, position: number, count: number): boolean {
  switch (rule.preset) {
    case 'all': return true
    case 'first': return position === 1
    case 'last': return position === count
    case 'odd': return position % 2 === 1
    case 'even': return position % 2 === 0
    case 'every': return !!rule.every && rule.every > 0 && (position - rule.offset) % rule.every === 0
    // Advanced expressions follow MTR's native priority: exact positions outrank
    // periodic matches, and the whitelist wins ties (including two unmatched lists).
    case 'custom': return expressionStrength(rule.whitelist, position, count) >= expressionStrength(rule.blacklist, position, count)
  }
}

/** The first instance faces +Z. Reversal swaps the leading and trailing coupling pads. */
export function arrangeConsist(train: TrainDefinition) {
  let rear = 0
  const result = train.previewConsist.flatMap((instance, index) => {
    const carriage = train.carriages.find(item => item.id === instance.carriageId)
    if (!carriage) return []
    const frontPad = instance.reversed ? carriage.couplingPadding2 : carriage.couplingPadding1
    const rearPad = instance.reversed ? carriage.couplingPadding1 : carriage.couplingPadding2
    const z = rear - frontPad - carriage.length / 2
    rear = z - carriage.length / 2 - rearPad
    return [{ carriage, reversed: instance.reversed, index, z }]
  })
  const center = rear / 2
  return result.map(item => ({ ...item, z: item.z - center }))
}
