import { describe, expect, test, vi } from 'vitest'

const { handle401, responseUse } = vi.hoisted(() => ({
  handle401: vi.fn(),
  responseUse: vi.fn(),
}))

vi.mock('@/lib/auth', () => ({
  handle401,
}))

vi.mock('axios', () => ({
  default: {
    create: () => ({
      interceptors: {
        response: {
          use: responseUse,
        },
      },
    }),
  },
}))

import '../api-client'

describe('api client auth interception', () => {
  test('delegates 401 responses to shared auth handling', async () => {
    const rejectHandler = responseUse.mock.calls[0][1] as (error: unknown) => Promise<never>
    const error = { response: { status: 401 } }

    await expect(rejectHandler(error)).rejects.toBe(error)
    expect(handle401).toHaveBeenCalledTimes(1)
  })
})
