import { describe, expect, it } from 'vitest'
import {
  localizeUpdateError,
  updateProgressLabel,
  type UpdateProgress,
} from '../src/composables/useUpdateProgress'

const t = (key: string, params?: Record<string, string | number>) =>
  params ? `${key}:${JSON.stringify(params)}` : key

function progress(overrides: Partial<UpdateProgress>): UpdateProgress {
  return {
    stage: 'checking',
    downloaded: 0,
    total: null,
    percent: null,
    ...overrides,
  }
}

describe('updateProgressLabel', () => {
  it('shows a numeric download percentage when content length is known', () => {
    expect(updateProgressLabel(progress({ stage: 'downloading', percent: 42 }), t))
      .toBe('toolbar.downloadingUpdatePercent:{"percent":42}')
  })

  it('uses stage labels when percentage is unavailable', () => {
    expect(updateProgressLabel(progress({ stage: 'downloading' }), t))
      .toBe('toolbar.downloadingUpdate')
    expect(updateProgressLabel(progress({ stage: 'verifying' }), t))
      .toBe('toolbar.verifyingUpdate')
  })
})

describe('localizeUpdateError', () => {
  it('maps stable timeout codes to translated messages', () => {
    expect(localizeUpdateError('UPDATE_CHECK_TIMEOUT', t)).toBe('toolbar.updateCheckTimeout')
    expect(localizeUpdateError('UPDATE_DOWNLOAD_TIMEOUT', t)).toBe('toolbar.updateDownloadTimeout')
  })

  it('preserves other updater errors', () => {
    expect(localizeUpdateError('signature mismatch', t)).toBe('signature mismatch')
  })
})
