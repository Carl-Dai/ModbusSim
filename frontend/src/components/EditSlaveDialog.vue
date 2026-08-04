<script setup lang="ts">
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n, showAlert } from 'shared-frontend'

const { t } = useI18n()

interface Props {
  show: boolean
  connectionId: string
  originalSlaveId: number
  initialName: string
}

const props = defineProps<Props>()
const emit = defineEmits<{
  close: []
  updated: [slaveId: number]
}>()

const slaveId = ref(1)
const slaveName = ref('')

watch(
  () => [props.show, props.originalSlaveId, props.initialName],
  () => {
    if (!props.show) return
    slaveId.value = props.originalSlaveId
    slaveName.value = props.initialName
  },
  { immediate: true },
)

async function submit() {
  if (!Number.isInteger(slaveId.value) || slaveId.value < 1 || slaveId.value > 247) {
    await showAlert(t('errors.invalidSlaveId'))
    return
  }
  try {
    await invoke('update_slave_device', {
      request: {
        connection_id: props.connectionId,
        original_slave_id: props.originalSlaveId,
        slave_id: slaveId.value,
        name: slaveName.value.trim(),
      },
    })
    emit('updated', slaveId.value)
    emit('close')
  } catch (error) {
    await showAlert(t('errors.operationFailed', { err: String(error) }))
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" class="modal-overlay" @click.self="emit('close')">
      <div class="modal-box">
        <div class="modal-title">{{ t('tree.editSlave') }}</div>
        <div class="modal-field">
          <label>{{ t('dialog.slaveId') }}</label>
          <input v-model.number="slaveId" type="number" min="1" max="247" />
        </div>
        <div class="modal-field">
          <label>{{ t('dialog.slaveName') }}</label>
          <input v-model="slaveName" type="text" :placeholder="t('dialog.slaveNamePlaceholder')" @keyup.enter="submit" />
        </div>
        <div class="modal-actions">
          <button class="modal-btn cancel" @click="emit('close')">{{ t('common.cancel') }}</button>
          <button class="modal-btn confirm" @click="submit">{{ t('common.save') }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-box { background: #1e1e2e; border: 1px solid #45475a; border-radius: 8px; padding: 20px; min-width: 320px; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
.modal-title { font-size: 14px; font-weight: 600; color: #cdd6f4; margin-bottom: 16px; }
.modal-field { margin-bottom: 14px; }
.modal-field label { display: block; font-size: 12px; color: #a6adc8; margin-bottom: 6px; }
.modal-field input { width: 100%; box-sizing: border-box; padding: 6px 10px; background: #313244; border: 1px solid #45475a; border-radius: 4px; color: #cdd6f4; font-size: 13px; outline: none; }
.modal-field input:focus { border-color: #89b4fa; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
.modal-btn { padding: 6px 16px; border: none; border-radius: 4px; font-size: 12px; cursor: pointer; }
.modal-btn.cancel { background: #313244; color: #a6adc8; }
.modal-btn.confirm { background: #89b4fa; color: #1e1e2e; font-weight: 600; }
</style>
