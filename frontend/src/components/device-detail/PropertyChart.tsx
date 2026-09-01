import { useMemo } from 'react'
import { Line } from 'react-chartjs-2'
import {
  Chart as ChartJS,
  LinearScale,
  LineController,
  LineElement,
  PointElement,
  Tooltip,
  type ChartData,
  type ChartOptions,
  type Plugin,
} from 'chart.js'
import type { PropertySeriesView } from '@/lib/api-generated/types.gen'
import { formatDatetime } from '@/lib/utils'
import { SERIES_COLORS } from './property-chart-model'

ChartJS.register(LinearScale, LineController, LineElement, PointElement, Tooltip)

// A gap between consecutive points larger than 2x the median interval breaks
// the line: an offline period stays visually empty instead of being bridged by
// a long misleading segment.
function withGapBreaks(points: { x: number; y: number }[]): { x: number; y: number | null }[] {
  if (points.length < 3) return points
  const intervals: number[] = []
  for (let i = 1; i < points.length; i++) {
    intervals.push(points[i].x - points[i - 1].x)
  }
  const sorted = [...intervals].sort((a, b) => a - b)
  const median = sorted[Math.floor(sorted.length / 2)]
  if (median <= 0) return points
  const threshold = median * 2
  const data: { x: number; y: number | null }[] = [points[0]]
  for (let i = 1; i < points.length; i++) {
    if (points[i].x - points[i - 1].x > threshold) {
      data.push({ x: points[i - 1].x + (points[i].x - points[i - 1].x) / 2, y: null })
    }
    data.push(points[i])
  }
  return data
}

function formatTick(value: number | string, spanMs: number): string {
  const d = new Date(Number(value))
  const pad = (n: number) => String(n).padStart(2, '0')
  const hm = `${pad(d.getHours())}:${pad(d.getMinutes())}`
  if (spanMs <= 48 * 3600 * 1000) return hm
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${hm}`
}

// react-chartjs-2 re-runs chart.update() whenever the data/options/plugins
// references change, so they are memoized to keep unrelated re-renders of the
// parent (typing in the custom range, chip toggles, background refetches) from
// re-laying-out an unchanged chart.
const NO_PLUGINS: Plugin<'line'>[] = []

export function PropertyChart({ series }: { series: PropertySeriesView[] }) {
  const single = series.length === 1
  const spanMs = useMemo(() => {
    let min = Infinity
    let max = -Infinity
    for (const s of series) {
      for (const p of s.points) {
        const t = new Date(p.time).getTime()
        if (t < min) min = t
        if (t > max) max = t
      }
    }
    return Number.isFinite(min) && Number.isFinite(max) ? max - min : 0
  }, [series])

  const data: ChartData<'line'> = useMemo(
    () => ({
      datasets: series.map((s, i) => {
        const color = SERIES_COLORS[i % SERIES_COLORS.length]
        return {
          label: s.key,
          data: withGapBreaks(
            s.points.map((p) => ({ x: new Date(p.time).getTime(), y: Number(p.value) }))
          ),
          borderColor: color,
          backgroundColor: color,
          yAxisID: single ? 'y' : `y${i}`,
          pointRadius: 1.5,
          pointHitRadius: 8,
          borderWidth: 1.5,
          spanGaps: false,
        }
      }),
    }),
    [series, single]
  )

  const options: ChartOptions<'line'> = useMemo(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      animation: false,
      interaction: { mode: 'nearest', intersect: false },
      plugins: {
        legend: { display: false },
        tooltip: {
          mode: 'nearest',
          intersect: false,
          callbacks: {
            title: (items) => {
              const x = items[0]?.parsed.x
              return typeof x === 'number' ? formatDatetime(new Date(x).toISOString()) : ''
            },
            label: (item) => `${item.dataset.label}: ${String(item.parsed.y)}`,
          },
        },
      },
      scales: {
        x: {
          type: 'linear',
          ticks: {
            maxTicksLimit: 8,
            callback: (value: number | string) => formatTick(value, spanMs),
          },
        },
        ...(single
          ? {
              y: {
                beginAtZero: false,
              },
            }
          : Object.fromEntries(
              series.map((_, i) => [
                `y${i}`,
                {
                  position: i < 2 ? ('left' as const) : ('right' as const),
                  ticks: { color: SERIES_COLORS[i % SERIES_COLORS.length] },
                  grid: { drawOnChartArea: i === 0 },
                },
              ])
            )),
      },
    }),
    [series, single, spanMs]
  )

  return (
    <div data-testid="property-chart-container" style={{ height: 320, width: '100%' }}>
      <Line data={data} options={options} plugins={NO_PLUGINS} />
    </div>
  )
}
