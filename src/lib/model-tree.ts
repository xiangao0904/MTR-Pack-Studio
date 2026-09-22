import type { ModelPartSummary } from './projects'

export interface PartTreeNode { id: string; name: string; partIds: string[]; children: PartTreeNode[] }

/** Preserve explicit paths; collect repeated name prefixes for flat mesh formats. */
export function buildPartTree(parts: ModelPartSummary[]): PartTreeNode[] {
  const counts = new Map<string, number>()
  for (const part of parts) { const prefix = part.name.split('_')[0]!; counts.set(prefix, (counts.get(prefix) || 0) + 1) }
  const roots: PartTreeNode[] = []
  for (const part of parts) {
    const segments = part.name.split(/\/|::/).filter(Boolean)
    if (segments.length === 1 && part.name.includes('_') && (counts.get(part.name.split('_')[0]!) || 0) > 1) segments.unshift(part.name.split('_')[0]!)
    let siblings = roots
    let path = ''
    for (const name of segments.slice(0, -1)) {
      path += `/${name}`
      let group = siblings.find(node => node.id === `group:${path}`)
      if (!group) { group = { id: `group:${path}`, name, partIds: [], children: [] }; siblings.push(group) }
      group.partIds.push(part.id); siblings = group.children
    }
    siblings.push({ id: part.id, name: segments.at(-1) || part.name, partIds: [part.id], children: [] })
  }
  return roots
}

export function flattenPartTree(nodes: PartTreeNode[], collapsed: Set<string>, query = '', depth = 0): (PartTreeNode & { depth: number })[] {
  const search = query.trim().toLowerCase()
  const matches = (node: PartTreeNode): boolean => node.name.toLowerCase().includes(search) || node.children.some(matches)
  return nodes.filter(node => !search || matches(node)).flatMap(node => [
    { ...node, depth },
    ...(node.children.length && (search || !collapsed.has(node.id)) ? flattenPartTree(node.children, collapsed, node.name.toLowerCase().includes(search) ? '' : query, depth + 1) : []),
  ])
}
