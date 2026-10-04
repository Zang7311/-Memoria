<!-- 《铃·记忆体》输入栏：textarea + 纸飞机发送按钮
     任务 4：Enter 发送、Shift+Enter 换行、发送中禁用、清空输入 -->
<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { useChatStore } from '../stores/chatStore'
import { useStreamRender } from '../composables/useStreamRender'
import { useSettingStore } from '../stores/settingStore'
import { ATTACHMENT_ACCEPT, readAttachment } from '../utils/attachments'
import type { Attachment } from '../types'

const chat = useChatStore()
const setting = useSettingStore()
const { send, sendAgent, cancelAgent } = useStreamRender()

const taRef = ref<HTMLTextAreaElement | null>(null)
const fileRef = ref<HTMLInputElement | null>(null)
const attachments = ref<Attachment[]>([])
const attachmentError = ref('')
const readingFiles = ref(false)

async function selectFiles(event: Event) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  readingFiles.value = true
  attachmentError.value = ''
  try {
    for (const file of files) {
      try { attachments.value.push(await readAttachment(file)) }
      catch (error) {
        attachmentError.value = error instanceof Error ? error.message : '附件读取失败，请重新选择'
      }
    }
  } finally {
    input.value = ''
    readingFiles.value = false
  }
}
// Agent 模式开关
// 持久化到 localStorage：用户开启过一次后，下次启动默认就是开启状态
const AGENT_MODE_KEY = 'ling_agent_mode_enabled'
const agentMode = ref(localStorage.getItem(AGENT_MODE_KEY) === '1')
watch(agentMode, (v) => {
  localStorage.setItem(AGENT_MODE_KEY, v ? '1' : '0')
})

// v0.6：悬浮球「双击快速提问」→ 唤起主窗口后聚焦输入框
onMounted(() => {
  listen('floating-quick-ask', () => {
    taRef.value?.focus()
  }).catch(() => { /* 非 Tauri 环境忽略 */ })
})

// 处理 Enter 键：Shift+Enter 换行，Enter 发送
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
}

// 发送逻辑：Agent 模式走 sendAgent，普通模式走 send
async function handleSend() {
  const content = chat.inputText.trim()
  if ((!content && !attachments.value.length) || chat.isLoading || readingFiles.value) return
  if (agentMode.value && !attachments.value.length) {
    await sendAgent(content)
  } else {
    const sent = await send(content, setting.depth, attachments.value.slice())
    if (sent === false && attachments.value.length) {
      attachmentError.value = '附件发送失败，请检查文件和模型配置后重试'
      return
    }
  }
  chat.inputText = ''
  attachments.value = []
  attachmentError.value = ''
}
</script>

<template>
  <div v-if="attachments.length || attachmentError || readingFiles" class="attachment-panel">
    <div class="attachment-tags">
      <span v-for="(attachment, index) in attachments" :key="index" class="attachment-tag">
        {{ attachment.name }}
        <button type="button" :disabled="chat.isLoading || readingFiles" :aria-label="'移除附件 ' + attachment.name" @click="attachments.splice(index, 1)">移除</button>
      </span>
    </div>
    <p v-if="readingFiles" role="status">正在读取附件</p>
    <p v-if="attachmentError" role="alert">{{ attachmentError }}</p>
    <p v-if="agentMode && attachments.length">带附件的消息使用普通对话发送</p>
  </div>
  <div class="chat-input">
    <input ref="fileRef" type="file" hidden multiple :accept="ATTACHMENT_ACCEPT" @change="selectFiles" />
    <button class="attachment-btn" type="button" :disabled="chat.isLoading || readingFiles" @click="fileRef?.click()">附件</button>
    <label class="agent-toggle" :class="{ active: agentMode }" title="开启后铃会自主调用工具完成任务">
      <input type="checkbox" v-model="agentMode" />
      Agent 模式
    </label>
    <textarea
      v-model="chat.inputText"
      ref="taRef"
      class="input-area"
      name="chat"
      :placeholder="agentMode ? '说出你的任务，铃会自己想办法完成…' : '说点什么…'"
      :disabled="chat.isLoading || readingFiles"
      rows="1"
      @keydown="onKeydown"
    ></textarea>
    <button
      v-if="agentMode && chat.isLoading"
      class="send-btn stop-btn"
      title="停止"
      @click="cancelAgent"
    >
      停止
    </button>
    <button
      v-else
      class="send-btn"
      :disabled="chat.isLoading || readingFiles || (!chat.inputText.trim() && !attachments.length)"
      title="发送"
      @click="handleSend"
    >
      发送
    </button>
  </div>
</template>

<style scoped>
.attachment-panel {
  padding: 8px 16px 0;
  color: var(--text-sub, #888);
  font-size: var(--fs-12, 12px);
}
.attachment-panel p { margin: 4px 0; }
.attachment-tags { display: flex; flex-wrap: wrap; gap: 6px; }
.attachment-tag {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  overflow-wrap: anywhere;
  padding: 4px 8px;
  border: 1px solid var(--border, #ccc);
  border-radius: 8px;
}
.attachment-tag button {
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  white-space: nowrap;
}
.attachment-btn {
  min-height: 40px;
  border: 1px solid var(--border, #ccc);
  border-radius: 8px;
  background: var(--input-bg, #fff);
  color: var(--text-main, #222);
  cursor: pointer;
}
.attachment-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.chat-input {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 8px 16px 22px;
  border-top: 1px solid var(--border, rgba(128, 128, 128, 0.2));
}
.agent-toggle {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--fs-12, 12px);
  color: var(--text-sub, #888);
  white-space: nowrap;
  cursor: pointer;
  user-select: none;
  padding: 4px 6px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
  transition: border-color 0.15s, color 0.15s;
}
.agent-toggle input {
  display: none;
}
.agent-toggle.active {
  color: var(--accent, #ff8fa3);
  border-color: var(--accent, #ff8fa3);
}
.input-area {
  flex: 1;
  resize: none;
  max-height: 120px;
  min-height: 40px;
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
  background: var(--input-bg, #ffffff);
  color: var(--text-main, #222);
  font-size: var(--fs-14);
  line-height: 1.5;
  font-family: inherit;
}
.input-area:focus {
  outline: none;
  border-color: var(--accent, #ffa7a7);
}
.input-area:disabled {
  opacity: 0.6;
}
.send-btn {
  width: 42px;
  height: 42px;
  border: none;
  border-radius: 12px;
  background: var(--accent, #ff8fa3);
  color: var(--text-user);
  font-size: var(--fs-13, 13px);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.12s ease;
}
.send-btn:hover:not(:disabled) {
  transform: scale(1.06);
}
.send-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.stop-btn {
  background: var(--danger, #d9534f);
  color: #fff;
  font-size: var(--fs-13, 13px);
  font-weight: 600;
  letter-spacing: 0.02em;
}
.stop-btn:hover {
  transform: scale(1.06);
}
</style>
