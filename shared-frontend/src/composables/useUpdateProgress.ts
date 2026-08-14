import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type UpdateProgressStage = 'idle' | 'checking' | 'downloading' | 'verifying' | 'ready'

export interface UpdateProgress {
  stage: UpdateProgressStage
  downloaded: number
  total: number | null
  percent: number | null
}

type Translate = (key: string, params?: Record<string, string | number>) => string

export function updateProgressLabel(progress: UpdateProgress, t: Translate): string {
  if (progress.stage === 'downloading') {
    return progress.percent === null
      ? t('toolbar.downloadingUpdate')
      : t('toolbar.downloadingUpdatePercent', { percent: progress.percent })
  }
  if (progress.stage === 'verifying') return t('toolbar.verifyingUpdate')
  return t('toolbar.checkingUpdate')
}

export function localizeUpdateError(error: unknown, t: Translate): string {
  const message = String(error)
  if (message.includes('UPDATE_CHECK_TIMEOUT')) return t('toolbar.updateCheckTimeout')
  if (message.includes('UPDATE_DOWNLOAD_TIMEOUT')) return t('toolbar.updateDownloadTimeout')
  return message
}

export function useUpdateProgress(t: Translate) {
  const progress = ref<UpdateProgress | null>(null)
  let unlisten: UnlistenFn | null = null
  let disposed = false

  onMounted(async () => {
    const stop = await listen<UpdateProgress>('update-progress', (event) => {
      const next = event.payload
      progress.value = next.stage === 'idle' || next.stage === 'ready' ? null : next
    })
    if (disposed) stop()
    else unlisten = stop
  })

  onBeforeUnmount(() => {
    disposed = true
    unlisten?.()
  })

  const active = computed(() => progress.value !== null)
  const label = computed(() =>
    progress.value ? updateProgressLabel(progress.value, t) : t('toolbar.checkUpdate'),
  )

  return { progress, active, label }
}
