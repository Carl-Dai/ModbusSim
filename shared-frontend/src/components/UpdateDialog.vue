<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from '../i18n'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  version: string
  notes: string
}>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'snooze'): void
}>()

const downloading = ref(false)
const progress = ref(0)
const error = ref<string | null>(null)
let unlisten: UnlistenFn | null = null

// --- Lightweight markdown rendering -----------------------------------------
// The release notes arrive as a Markdown CHANGELOG section. Rendering them as
// raw <pre> text dumps `###`, `**` and `-` literally on screen; instead parse
// the small subset actually used (headings, bullets, bold, inline code) into a
// structured token tree and render it with plain Vue templates (no v-html).

type Span = { text: string; bold?: boolean; code?: boolean }
type Block =
  | { kind: 'h'; level: number; spans: Span[] }
  | { kind: 'li'; spans: Span[] }
  | { kind: 'p'; spans: Span[] }
  | { kind: 'quote'; spans: Span[] }
  | { kind: 'hr' }

function parseInline(text: string): Span[] {
  const spans: Span[] = []
  const re = /(\*\*[^*]+\*\*|`[^`]+`)/g
  let last = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) spans.push({ text: text.slice(last, m.index) })
    const tok = m[0]
    if (tok.startsWith('**')) spans.push({ text: tok.slice(2, -2), bold: true })
    else spans.push({ text: tok.slice(1, -1), code: true })
    last = m.index + tok.length
  }
  if (last < text.length) spans.push({ text: text.slice(last) })
  return spans.length ? spans : [{ text }]
}

const noteBlocks = computed<Block[]>(() => {
  const out: Block[] = []
  for (const raw of (props.notes || '').split('\n')) {
    const line = raw.trim()
    if (!line) continue
    if (/^[-*_]{3,}$/.test(line)) { out.push({ kind: 'hr' }); continue }
    const h = line.match(/^(#{1,6})\s+(.*)$/)
    if (h) { out.push({ kind: 'h', level: h[1].length, spans: parseInline(h[2]) }); continue }
    const li = line.match(/^[-*]\s+(.*)$/)
    if (li) { out.push({ kind: 'li', spans: parseInline(li[1]) }); continue }
    const q = line.match(/^>\s?(.*)$/)
    if (q) { out.push({ kind: 'quote', spans: parseInline(q[1]) }); continue }
    out.push({ kind: 'p', spans: parseInline(line) })
  }
  return out
})

// --- Actions ----------------------------------------------------------------

async function install() {
  error.value = null
  downloading.value = true
  progress.value = 0
  unlisten = await listen<number>('update-progress', (e) => {
    progress.value = e.payload
  })
  try {
    await invoke('install_update')
  } catch (e: any) {
    error.value = String(e)
    downloading.value = false
  } finally {
    if (unlisten) { unlisten(); unlisten = null }
  }
}

function later() {
  emit('snooze')
  emit('close')
}

function onBackdrop() {
  // Backdrop / Esc dismissal is a snooze, but never interrupt a download.
  if (downloading.value) return
  if (error.value) emit('close')
  else later()
}

function onKeydown(e: KeyboardEvent) {
  if (props.visible && e.key === 'Escape') onBackdrop()
}
onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  if (unlisten) { unlisten(); unlisten = null }
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-pop">
      <div v-if="visible" class="upd-backdrop" @mousedown.self="onBackdrop">
        <div class="upd-dialog" role="dialog" aria-modal="true" aria-labelledby="upd-title">
          <!-- Header -->
          <div class="upd-header">
            <div class="upd-titles">
              <div id="upd-title" class="upd-title">{{ t('update.available') }}</div>
              <div class="upd-subtitle">{{ t('update.newVersion', { version }) }}</div>
            </div>
            <span class="upd-badge">v{{ version }}</span>
          </div>

          <!-- Body -->
          <div class="upd-body">
            <div class="upd-section-label">{{ t('update.changelog') }}</div>
            <div class="upd-notes" tabindex="0">
              <template v-for="(blk, i) in noteBlocks" :key="i">
                <hr v-if="blk.kind === 'hr'" class="upd-hr" />
                <component
                  :is="'h' + Math.min(blk.level + 2, 6)"
                  v-else-if="blk.kind === 'h'"
                  class="upd-h"
                >
                  <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                    <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                    <template v-else>{{ s.text }}</template>
                  </span>
                </component>
                <div v-else-if="blk.kind === 'li'" class="upd-li">
                  <span class="upd-bullet" aria-hidden="true"></span>
                  <span class="upd-li-text">
                    <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                      <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                      <template v-else>{{ s.text }}</template>
                    </span>
                  </span>
                </div>
                <blockquote v-else-if="blk.kind === 'quote'" class="upd-quote">
                  <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                    <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                    <template v-else>{{ s.text }}</template>
                  </span>
                </blockquote>
                <p v-else class="upd-p">
                  <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                    <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                    <template v-else>{{ s.text }}</template>
                  </span>
                </p>
              </template>
            </div>

            <!-- Download progress -->
            <div v-if="downloading" class="upd-progress" aria-live="polite">
              <div class="upd-progress-row">
                <span>{{ t('update.downloading', { pct: progress }) }}</span>
                <span class="upd-progress-pct">{{ progress }}%</span>
              </div>
              <div class="upd-track">
                <div class="upd-fill" :style="{ transform: `scaleX(${progress / 100})` }"></div>
              </div>
            </div>

            <!-- Error -->
            <div v-if="error" class="upd-error" role="alert">
              <div class="upd-error-title">{{ t('update.failedTitle') }}</div>
              <pre class="upd-error-msg">{{ error }}</pre>
            </div>
          </div>

          <!-- Footer -->
          <div class="upd-footer">
            <template v-if="!downloading && !error">
              <button class="btn btn-ghost" @click="later">{{ t('update.later') }}</button>
              <button class="btn btn-primary" @click="install">{{ t('update.installNow') }}</button>
            </template>
            <template v-else-if="error">
              <button class="btn btn-ghost" @click="$emit('close')">{{ t('update.close') }}</button>
              <button class="btn btn-primary" @click="install">{{ t('update.retry') }}</button>
            </template>
            <span v-else class="upd-footer-hint">{{ t('update.downloading', { pct: progress }) }}</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.upd-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(17, 17, 27, 0.66);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2100;
}
.upd-dialog {
  display: flex;
  flex-direction: column;
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 10px;
  width: 540px;
  max-width: 92vw;
  max-height: 78vh;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.55);
  overflow: hidden;
}

/* Header */
.upd-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 20px 14px;
  border-bottom: 1px solid #313244;
  background: #181825;
}
.upd-title { font-size: 15px; font-weight: 600; color: #cdd6f4; }
.upd-subtitle { font-size: 12px; color: #a6adc8; margin-top: 3px; }
.upd-badge {
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: #89b4fa;
  background: rgba(137, 180, 250, 0.14);
  border: 1px solid rgba(137, 180, 250, 0.32);
  padding: 3px 9px;
  border-radius: 999px;
}

/* Body */
.upd-body {
  padding: 14px 20px;
  overflow-y: auto;
}
.upd-section-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: #a6adc8;
  margin-bottom: 8px;
}
.upd-notes {
  background: #181825;
  border: 1px solid #313244;
  border-radius: 8px;
  padding: 12px 14px;
  max-height: 320px;
  overflow-y: auto;
  font-size: 13px;
  line-height: 1.6;
  color: #bac2de;
}
.upd-notes:focus-visible {
  outline: 2px solid #89b4fa;
  outline-offset: -1px;
}

/* Rendered markdown */
.upd-h {
  margin: 14px 0 6px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: #b4befe;
}
.upd-h:first-child { margin-top: 0; }
.upd-p { margin: 6px 0; }
.upd-li {
  display: flex;
  gap: 8px;
  margin: 5px 0;
  align-items: baseline;
}
.upd-bullet {
  flex-shrink: 0;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #89b4fa;
  transform: translateY(-1px);
}
.upd-li-text { flex: 1; min-width: 0; }
.upd-notes .b { font-weight: 600; color: #cdd6f4; }
.upd-code {
  font-family: ui-monospace, 'SF Mono', Consolas, monospace;
  font-size: 12px;
  color: #fab387;
  background: #11111b;
  border: 1px solid #313244;
  border-radius: 4px;
  padding: 0 4px;
}
.upd-quote {
  margin: 6px 0;
  padding: 4px 10px;
  border-left: 2px solid #585b70;
  color: #a6adc8;
}
.upd-hr {
  border: none;
  border-top: 1px solid #313244;
  margin: 12px 0;
}

/* Progress */
.upd-progress { margin-top: 14px; }
.upd-progress-row {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: #bac2de;
  margin-bottom: 6px;
}
.upd-progress-pct {
  font-variant-numeric: tabular-nums;
  color: #89b4fa;
  font-weight: 600;
}
.upd-track {
  height: 6px;
  border-radius: 999px;
  background: #313244;
  overflow: hidden;
}
.upd-fill {
  height: 100%;
  border-radius: 999px;
  background: #89b4fa;
  transform-origin: left;
  transition: transform 200ms ease-out;
}

/* Error */
.upd-error {
  margin-top: 14px;
  padding: 10px 12px;
  background: rgba(243, 139, 168, 0.1);
  border: 1px solid rgba(243, 139, 168, 0.32);
  border-radius: 8px;
}
.upd-error-title { font-size: 13px; font-weight: 600; color: #f38ba8; }
.upd-error-msg {
  margin: 6px 0 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, 'SF Mono', Consolas, monospace;
  font-size: 11px;
  color: #a6adc8;
}

/* Footer */
.upd-footer {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid #313244;
  background: #181825;
}
.upd-footer-hint { font-size: 12px; color: #a6adc8; }
.btn {
  padding: 7px 18px;
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  transition: background 140ms ease, border-color 140ms ease;
}
.btn:focus-visible { outline: 2px solid #89b4fa; outline-offset: 2px; }
.btn-primary { background: #89b4fa; color: #11111b; }
.btn-primary:hover { background: #74c7ec; }
.btn-ghost {
  background: transparent;
  color: #bac2de;
  border-color: #45475a;
}
.btn-ghost:hover { background: #313244; color: #cdd6f4; }

/* Dark scrollbar for the scroll regions */
.upd-notes::-webkit-scrollbar,
.upd-body::-webkit-scrollbar { width: 8px; }
.upd-notes::-webkit-scrollbar-thumb,
.upd-body::-webkit-scrollbar-thumb {
  background: #45475a;
  border-radius: 999px;
}
.upd-notes::-webkit-scrollbar-thumb:hover,
.upd-body::-webkit-scrollbar-thumb:hover { background: #585b70; }
.upd-notes::-webkit-scrollbar-track,
.upd-body::-webkit-scrollbar-track { background: transparent; }

@media (prefers-reduced-motion: reduce) {
  .upd-fill { transition: none; }
}
</style>
