import { describe, expect, test } from 'vitest'
import { completeAuthCallback } from '../callback'

describe('auth callback route', () => {
  test('stores the Herald token cookie and returns the redirect target', () => {
    const redirect = completeAuthCallback('?token=session-123&redirect=%2Fdevices%3Fpage%3D2')

    expect(document.cookie).toContain('X-Auth=session-123')
    expect(redirect).toBe('/devices?page=2')
  })
})
