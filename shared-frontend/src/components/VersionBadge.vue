<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'

const REPO_URL = 'https://github.com/Karl-Dai/ModbusSim'
const version = ref('')

onMounted(async () => {
  try { version.value = await getVersion() } catch { version.value = '' }
})

// In a Tauri webview window.open can't reach the OS browser, so go through the
// opener plugin via raw invoke (shared-frontend only depends on @tauri-apps/api).
// Outside Tauri (plain vite dev in browser) invoke throws — fall back to window.open.
async function openRepo() {
  try {
    await invoke('plugin:opener|open_url', { url: REPO_URL })
  } catch {
    window.open(REPO_URL, '_blank')
  }
}
</script>

<template>
  <div class="version-badge">
    <span v-if="version" class="version-text" :title="`v${version}`">v{{ version }}</span>
    <button
      type="button"
      class="github-link"
      :title="REPO_URL"
      :aria-label="REPO_URL"
      @click="openRepo"
    >
      <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">
        <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38
                 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13
                 -.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66
                 .07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15
                 -.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0
                 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82
                 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01
                 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"/>
      </svg>
    </button>
  </div>
</template>

<style scoped>
.version-badge {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 0 4px;
  font-size: 11px;
  color: #6c7086;
  font-variant-numeric: tabular-nums;
}
.version-text {
  padding: 2px 4px;
  line-height: 1;
  white-space: nowrap;
}
.github-link {
  display: inline-flex;
  align-items: center;
  padding: 3px 4px;
  margin: 0;
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  border-radius: 4px;
  line-height: 1;
}
.github-link:hover { color: #cdd6f4; background: #313244; }
.github-link svg { display: block; }
</style>
