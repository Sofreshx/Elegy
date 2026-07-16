import { describe, expect, it } from 'vitest'
import { resolveAccountCenterMode } from './mode'

describe('resolveAccountCenterMode', () => {
  it('uses embedded chrome for the Holon host hint', () => {
    expect(resolveAccountCenterMode('?embed=holon')).toBe('embedded')
  })

  it('keeps direct browser launches standalone', () => {
    expect(resolveAccountCenterMode('')).toBe('standalone')
  })
})
