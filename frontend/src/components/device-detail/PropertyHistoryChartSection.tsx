import { useEffect, useMemo, useRef } from 'react'
import { usePropertyHistoryKeys, usePropertyHistorySeries } from '@/hooks/useProperties'
import type { PropertySeriesView } from '@/lib/api-generated/types.gen'
import { extractErrorMessage, formatDatetime, toKebabKey } from '@/lib/utils'
import { PropertyChart } from './PropertyChart'
import { selectControlStyle } from './styles'
import {
  MAX_CHART_KEYS,
  SERIES_COLORS,
  presetRange,
  type ChartRange,
  type RangePreset,
} from './property-chart-model'

const inputStyle: React.CSSProperties = {
  ...selectControlStyle,
  height: '28px',
}

const chipBase: React.CSSProperties = {
  borderRadius: '9999px',
  padding: '2px 10px',
  fontSize: '12px',
  border: '1px solid var(--color-border)',
  background: 'var(--color-surface-1)',
  color: 'var(--color-text-primary)',
  cursor: 'pointer',
}

const noteStyle: React.CSSProperties = {
  fontSize: '13px',
  padding: '16px 0',
}

const errorStyle: React.CSSProperties = {
  color: '#dc2626',
  fontSize: '13px',
  padding: '8px 0',
}

interface Props {
  productId: string
  deviceId: string
  selectedKeys: string[]
  onSelectedKeysChange: (keys: string[]) => void
  range: ChartRange
  onRangeChange: (range: ChartRange) => void
  onSwitchToTable: () => void
}

export function PropertyHistoryChartSection({
  productId,
  deviceId,
  selectedKeys,
  onSelectedKeysChange,
  range,
  onRangeChange,
  onSwitchToTable,
}: Props) {
  const keysQuery = usePropertyHistoryKeys({ product_id: productId, device_id: deviceId })
  const keys = useMemo(() => keysQuery.data?.data ?? [], [keysQuery.data])

  // Default selection: the most active numeric key (keys come sorted by
  // sampleCount desc). Applied once per mount so a deliberate empty selection
  // by the user is never overwritten.
  const selectionInitialized = useRef(false)
  useEffect(() => {
    if (!selectionInitialized.current && keysQuery.isSuccess && keys.length > 0) {
      selectionInitialized.current = true
      if (selectedKeys.length === 0) {
        onSelectedKeysChange([keys[0].key])
      }
    }
  }, [keysQuery.isSuccess, keys, selectedKeys.length, onSelectedKeysChange])

  const startMs = Date.parse(range.start)
  const endMs = Date.parse(range.end)
  const rangeValid =
    range.preset !== 'custom' ||
    (range.start !== '' &&
      range.end !== '' &&
      !Number.isNaN(startMs) &&
      !Number.isNaN(endMs) &&
      startMs < endMs)

  const seriesEnabled = selectedKeys.length > 0 && rangeValid
  const seriesQuery = usePropertyHistorySeries(
    {
      product_id: productId,
      device_id: deviceId,
      keys: selectedKeys,
      start_time: new Date(startMs).toISOString(),
      end_time: new Date(endMs).toISOString(),
    },
    { enabled: seriesEnabled }
  )

  const series: PropertySeriesView[] = seriesQuery.data?.data ?? []
  const emptyKeys = new Set(series.filter((s) => s.totalPoints === 0).map((s) => s.key))
  const allEmpty = series.length > 0 && series.every((s) => s.totalPoints === 0)

  function toggleKey(key: string) {
    if (selectedKeys.includes(key)) {
      onSelectedKeysChange(selectedKeys.filter((k) => k !== key))
    } else if (selectedKeys.length < MAX_CHART_KEYS) {
      onSelectedKeysChange([...selectedKeys, key])
    }
  }

  function selectPreset(preset: string) {
    if (preset === 'custom') {
      onRangeChange({ preset: 'custom', start: range.start, end: range.end })
    } else {
      onRangeChange(presetRange(preset as Exclude<RangePreset, 'custom'>))
    }
  }

  return (
    <div className="space-y-3">
      {keysQuery.isLoading && <p data-testid="property-chart-loading">Loading properties…</p>}
      {keysQuery.isError && (
        <div>
          <p data-testid="property-chart-error" style={errorStyle}>
            Failed to load numeric properties: {extractErrorMessage(keysQuery.error)}
          </p>
          <button onClick={() => keysQuery.refetch()}>Retry</button>
        </div>
      )}
      {keysQuery.isSuccess && keys.length === 0 && (
        <div data-testid="property-chart-keys-empty">
          <p style={noteStyle}>
            该设备近 30 天没有可图表化的数值属性上报，可在表格视图查看原始记录。
          </p>
          <button onClick={onSwitchToTable}>查看表格</button>
        </div>
      )}
      {keysQuery.isSuccess && keys.length > 0 && (
        <>
          <div className="flex flex-wrap gap-2">
            {keys.map((k) => {
              const index = selectedKeys.indexOf(k.key)
              const selected = index >= 0
              const disabled = !selected && selectedKeys.length >= MAX_CHART_KEYS
              const color = SERIES_COLORS[index % SERIES_COLORS.length]
              return (
                <button
                  key={k.key}
                  data-testid={`property-chart-key-toggle-${toKebabKey(k.key)}`}
                  onClick={() => toggleKey(k.key)}
                  disabled={disabled}
                  title={disabled ? `最多同时对比 ${MAX_CHART_KEYS} 个属性` : undefined}
                  style={{
                    ...chipBase,
                    ...(selected
                      ? { borderColor: color, background: color, color: '#ffffff' }
                      : {}),
                    ...(disabled ? { opacity: 0.4, cursor: 'not-allowed' } : {}),
                  }}
                >
                  {k.key} ×{k.sampleCount}
                  {emptyKeys.has(k.key) && (
                    <span style={{ marginLeft: 6, opacity: 0.8 }}>无数据</span>
                  )}
                </button>
              )
            })}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <select
              data-testid="property-chart-range-select"
              value={range.preset}
              onChange={(e) => selectPreset(e.target.value)}
              style={selectControlStyle}
            >
              <option value="1h">Range: 1h</option>
              <option value="24h">Range: 24h</option>
              <option value="7d">Range: 7d</option>
              <option value="30d">Range: 30d</option>
              <option value="custom">Custom</option>
            </select>
            {range.preset === 'custom' ? (
              <>
                <input
                  data-testid="property-chart-start-input"
                  type="datetime-local"
                  value={range.start}
                  onChange={(e) => onRangeChange({ ...range, start: e.target.value })}
                  style={inputStyle}
                />
                <span>→</span>
                <input
                  data-testid="property-chart-end-input"
                  type="datetime-local"
                  value={range.end}
                  onChange={(e) => onRangeChange({ ...range, end: e.target.value })}
                  style={inputStyle}
                />
                {!rangeValid && (
                  <span style={{ color: '#dc2626', fontSize: '12px' }}>结束时间需晚于起始时间</span>
                )}
              </>
            ) : (
              !Number.isNaN(startMs) &&
              !Number.isNaN(endMs) && (
                <span style={{ fontSize: '12px', opacity: 0.7 }}>
                  {formatDatetime(new Date(startMs).toISOString())} →{' '}
                  {formatDatetime(new Date(endMs).toISOString())}
                </span>
              )
            )}
          </div>

          {series
            .filter((s) => s.downsampled)
            .map((s) => (
              <p
                key={s.key}
                data-testid={`property-chart-downsample-note-${toKebabKey(s.key)}`}
                style={{ fontSize: '12px', opacity: 0.8 }}
              >
                已降精度：{s.key} 共 {s.totalPoints} 条记录，显示 {s.points.length} 个抽样点（每{' '}
                {s.stride} 条取 1 条）
              </p>
            ))}

          {seriesQuery.isLoading && <p data-testid="property-chart-loading">Loading chart…</p>}
          {seriesQuery.isError && (
            <div>
              <p data-testid="property-chart-error" style={errorStyle}>
                Failed to load chart data: {extractErrorMessage(seriesQuery.error)}
              </p>
              <button onClick={() => seriesQuery.refetch()}>Retry</button>
            </div>
          )}
          {seriesQuery.isSuccess && selectedKeys.length === 0 && (
            <p data-testid="property-chart-no-selection" style={noteStyle}>
              选择一个属性开始查看趋势。
            </p>
          )}
          {seriesQuery.isSuccess && selectedKeys.length > 0 && allEmpty && (
            <p data-testid="property-chart-empty" style={noteStyle}>
              所选时间范围内没有数据，可换时间范围或到表格视图核对。
            </p>
          )}
          {seriesQuery.isSuccess && selectedKeys.length > 0 && !allEmpty && series.length > 0 && (
            <PropertyChart series={series} />
          )}
        </>
      )}
    </div>
  )
}
