/** Fade projected mesh edges before their spacing becomes smaller than a screen pixel. */
export function wireframeOpacity(projectedDiameter: number, triangleCount: number): number {
  if (!Number.isFinite(projectedDiameter) || projectedDiameter <= 0) return 0
  if (!Number.isFinite(triangleCount) || triangleCount <= 0) return 0.85
  const spacing = projectedDiameter / Math.sqrt(triangleCount / 2)
  const visibility = Math.min(1, Math.max(0, (spacing - 1.5) / 3))
  return 0.85 * visibility * visibility
}
