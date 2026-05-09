import { describe, expect, it } from 'vitest'
import { formatVersion, parseVersion } from '../version'

describe('formatVersion', () => {
  it('formats 0 as "0.0.0"', () => {
    expect(formatVersion(0)).toBe('0.0.0')
  })

  it('formats 100000 as "1.0.0"', () => {
    expect(formatVersion(100000)).toBe('1.0.0')
  })

  it('formats 101001 as "1.1.1"', () => {
    expect(formatVersion(101001)).toBe('1.1.1')
  })

  it('formats 102003 as "1.2.3"', () => {
    expect(formatVersion(102003)).toBe('1.2.3')
  })

  it('formats 99999999 as "999.99.999" at boundary', () => {
    expect(formatVersion(99999999)).toBe('999.99.999')
  })
})

describe('parseVersion', () => {
  it('parses "1.2.3" as 102003', () => {
    expect(parseVersion('1.2.3')).toBe(102003)
  })

  it('parses "0.0.0" as 0', () => {
    expect(parseVersion('0.0.0')).toBe(0)
  })

  it('parses "99.99.999" as 9999999 at boundary', () => {
    expect(parseVersion('99.99.999')).toBe(9999999)
  })

  it('throws for less than 3 parts', () => {
    expect(() => parseVersion('1.2')).toThrow(
      "Invalid version string format. Expected 'major.minor.patch'"
    )
  })

  it('throws for non-numeric parts', () => {
    expect(() => parseVersion('a.b.c')).toThrow(
      "Invalid version string format. Expected 'major.minor.patch'"
    )
  })

  it('throws for more than 3 parts', () => {
    expect(() => parseVersion('1.2.3.4')).toThrow(
      "Invalid version string format. Expected 'major.minor.patch'"
    )
  })

  it('throws when major > 99', () => {
    expect(() => parseVersion('100.0.0')).toThrow('Version component out of valid range.')
  })

  it('throws when minor > 99', () => {
    expect(() => parseVersion('0.100.0')).toThrow('Version component out of valid range.')
  })

  it('throws when patch > 999', () => {
    expect(() => parseVersion('0.0.1000')).toThrow('Version component out of valid range.')
  })
})

describe('roundtrip', () => {
  it('parseVersion(formatVersion(n)) === n for various integers', () => {
    const values = [0, 1, 999, 1000, 101001, 102003, 500500, 9999999]
    for (const n of values) {
      expect(parseVersion(formatVersion(n))).toBe(n)
    }
  })

  it('formatVersion(parseVersion(s)) === s for various valid strings', () => {
    const strings = ['0.0.0', '0.0.1', '1.0.0', '1.2.3', '99.99.999']
    for (const s of strings) {
      expect(formatVersion(parseVersion(s))).toBe(s)
    }
  })
})
