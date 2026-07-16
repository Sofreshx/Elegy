export type AccountCenterMode = 'standalone' | 'embedded'

export function resolveAccountCenterMode(search: string): AccountCenterMode {
  const embed = new URLSearchParams(search).get('embed')
  return embed === '1' || embed === 'holon' ? 'embedded' : 'standalone'
}
