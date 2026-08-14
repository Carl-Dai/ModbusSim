<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'shared-frontend'

const { t } = useI18n()

interface Props { connectionId: string | null; slaveId: number | null }
defineProps<Props>()
const emit = defineEmits<{ (e: 'mutated'): void }>()

/** Window event keeping the toolbar switch in sync with the Simulation
 *  Settings drawer (which can start the mutation engine on its own). */
const MUTATION_RUNNING_EVENT = 'modbussim:mutation-running'

// Master switch for backend point-level mutation. The timer only refreshes
// displayed values; mutation scheduling itself is entirely backend-driven.
const active = ref(false)
let refreshTimer: number | null = null

async function toggle() {
  if (active.value) await stop()
  else await start()
}

async function start() {
  try {
    await invoke('set_mutation_running', { running: true })
    active.value = true
    notifyRunning(true)
    emit('mutated')
    scheduleRefresh()
  } catch (e) {
    console.error('start mutation failed:', e)
  }
}

async function stop() {
  try {
    await invoke('set_mutation_running', { running: false })
  } catch (e) {
    console.error('stop mutation failed:', e)
  }
  active.value = false
  notifyRunning(false)
  clearRefresh()
}

function scheduleRefresh() {
  clearRefresh()
  refreshTimer = window.setInterval(() => emit('mutated'), 2000)
}

function clearRefresh() {
  if (refreshTimer !== null) {
    clearInterval(refreshTimer)
    refreshTimer = null
  }
}

function notifyRunning(running: boolean) {
  window.dispatchEvent(new CustomEvent(MUTATION_RUNNING_EVENT, { detail: { running } }))
}

function handleRunningEvent(event: Event) {
  const detail = (event as CustomEvent<{ running: boolean }>).detail
  if (!detail) return
  if (detail.running && !active.value) {
    active.value = true
    emit('mutated')
    scheduleRefresh()
  } else if (!detail.running && active.value) {
    active.value = false
    clearRefresh()
  }
}

onMounted(() => window.addEventListener(MUTATION_RUNNING_EVENT, handleRunningEvent))
onUnmounted(() => {
  window.removeEventListener(MUTATION_RUNNING_EVENT, handleRunningEvent)
  clearRefresh()
})
</script>

<template>
  <div class="mutation-group">
    <button
      :class="['toolbar-btn', { 'btn-mutation-active': active }]"
      @click="toggle"
      :disabled="!connectionId"
      :title="t('toolbar.randomMutation')"
    >
      <span class="toolbar-label">{{ active ? t('toolbar.stopMutation') : t('toolbar.randomMutation') }}</span>
    </button>
  </div>
</template>

<style scoped>
.mutation-group { display: flex; align-items: center; gap: 4px; }
.toolbar-btn { display: flex; align-items: center; gap: 4px; padding: 4px 10px; border: none; background: transparent; color: #cdd6f4; cursor: pointer; border-radius: 4px; font-size: 12px; white-space: nowrap; }
.toolbar-btn:hover:not(:disabled) { background: #313244; }
.toolbar-btn:disabled { opacity: 0.4; cursor: default; }
.toolbar-btn.btn-mutation-active { background: #a6e3a1; color: #1e1e2e; font-weight: 600; }
.toolbar-btn.btn-mutation-active:hover { background: #94e2d5; }
</style>
