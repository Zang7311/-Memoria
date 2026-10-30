<!-- 《铃·记忆体》输入区（现代 composer）：
     一块 raised 圆角容器 + 无边框 textarea + 底栏（附件 / Agent 模式 / 发送）
     保留：Enter 发送、Shift+Enter 换行、发送中禁用、清空输入、附件选择与删除、
          拖放附件（DROP_EVENT）、attachmentError 提示 -->
<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { useChatStore } from '../stores/chatStore'
import { useStreamRender } from '../composables/useStreamRender'
import { useSettingStore } from '../stores/settingStore'
import { ATTACHMENT_ACCEPT, readAttachment } from '../utils/attachments'
import { DROP_EVENT, type DropResult } from '../utils/fileDrop'
import type { Attachment } from '../types'
import AppButton from './ui/AppButton.vue'
import TheIcon from './TheIcon.vue'

const chat = useChatStore()
const setting = useSettingStore()
const { send, sendAgent, cancelAgent } = useStreamRender()

const taRef = ref<HTMLTextAreaElement | null>(null)
const fileRef = ref<HTMLInputElement | null>(null)
const attachments = ref<Attachment[]>([])
const attachmentError = ref('')
const readingFiles = ref(false)
// 输入框最大高度，超过后内部滚动
const MAX_INPUT_H = 120

// 桌面文件拖进窗口：后端读完内容后广播过来，这里追加到待发附件（跟「选择文件」同一个列表）
function onDroppedFiles(event: Event) {
  const result = (event as CustomEvent<DropResult>).detail
  if (!result) return
  for (const file of result.ok) {
    if (!attachments.value.some((a) => a.name === file.name && a.size === file.size)) {
      attachments.value.push(file)
    }
  }
  attachmentError.value = result.failed.length ? result.failed.join('；') : ''
}
onMounted(() => window.addEventListener(DROP_EVENT, onDroppedFiles))
onUnmounted(() => window.removeEventListener(DROP_EVENT, onDroppedFiles))

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

// Agent 模式开关：持久化到 localStorage，开启过一次后下次启动默认开启
const AGENT_MODE_KEY = 'ling_agent_mode_enabled'
const agentMode = ref(localStorage.getItem(AGENT_MODE_KEY) === '1')
watch(agentMode, (v) => {
  localStorage.setItem(AGENT_MODE_KEY, v ? '1' : '0')
})
function toggleAgentMode() {
  agentMode.value = !agentMode.value
}

// 输入框跟着内容长高，到上限后内部滚动
function autoGrow() {
  const el = taRef.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, MAX_INPUT_H)}px`
}
watch(() => chat.inputText, () => nextTick(autoGrow))

// v0.6：悬浮球「双击快速提问」→ 唤起主窗口后聚焦输入框
onMounted(() => {
  autoGrow()
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
  <div class="composer-box">
    <!-- 附件区 -->
    <div v-if="attachments.length || attachmentError || readingFiles" class="attach-panel">
      <div class="attach-tags">
        <span v-for="(attachment, index) in attachments" :key="index" class="attach-tag">
          <span class="attach-name">{{ attachment.name }}</span>
          <button
            type="button"
            class="attach-del"
            :disabled="chat.isLoading || readingFiles"
            :aria-label="'移除附件 ' + attachment.name"
            @click="attachments.splice(index, 1)"
          >移除</button>
        </span>
      </div>
      <p v-if="readingFiles" class="attach-note" role="status">正在读取附件</p>
      <p v-if="attachmentError" class="attach-error" role="alert">{{ attachmentError }}</p>
      <p v-if="agentMode && attachments.length" class="attach-note">带附件的消息使用普通对话发送</p>
    </div>

    <!-- 正文输入 -->
    <textarea
      ref="taRef"
      v-model="chat.inputText"
      class="ta"
      name="chat"
      :placeholder="agentMode ? '说出你的任务，铃会自己想办法完成…' : '说点什么…'"
      :disabled="chat.isLoading || readingFiles"
      rows="1"
      @keydown="onKeydown"
    ></textarea>

    <!-- 底栏 -->
    <div class="foot">
      <input ref="fileRef" type="file" hidden multiple :accept="ATTACHMENT_ACCEPT" @change="selectFiles" />
      <button
        type="button"
        class="tool"
        title="选择附件"
        :disabled="chat.isLoading || readingFiles"
        @click="fileRef?.click()"
      >
        <TheIcon name="file" :size="14" />
        <span>附件</span>
      </button>
      <button
        type="button"
        class="agent"
        :class="{ on: agentMode }"
        :aria-pressed="agentMode"
        title="开启后铃会自主调用工具完成任务"
        @click="toggleAgentMode"
      >Agent 模式</button>
      <span class="foot-gap" />
      <AppButton
        v-if="agentMode && chat.isLoading"
        class="stop-btn"
        title="停止"
        @click="cancelAgent"
      >停止</AppButton>
      <AppButton
        v-else
        variant="primary"
        :disabled="chat.isLoading || readingFiles || (!chat.inputText.trim() && !attachments.length)"
        title="发送"
        @click="handleSend"
      >
        <TheIcon name="send" :size="14" />
        <span>发送</span>
      </AppButton>
    </div>
  </div>
</template>

<style scoped>
/* 外层容器：raised 底 + 大圆角，聚焦时只换描边色 */
.composer-box {
  width: 100%;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  background: var(--ui-raised);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-lg);
  transition: border-color var(--ui-dur-1) var(--ui-ease);
}
.composer-box:focus-within {
  border-color: var(--ui-accent-line);
}

/* 正文输入：无边框无底色 */
.ta {
  display: block;
  width: 100%;
  min-height: 44px;
  max-height: 120px;
  padding: var(--ui-sp-1) 0;
  border: none;
  background: transparent;
  color: var(--ui-text);
  font-family: inherit;
  font-size: var(--ui-fs-14);
  line-height: var(--ui-line);
  resize: none;
  outline: none;
}
/* 主题深化层会给全局 textarea 加底色/描边（tokens.css 里带 !important），
   这里用更高的选择器优先级压回去，保证 composer 里就是一块干净文字区 */
.composer-box .ta {
  border: none !important;
  background: transparent !important;
  border-radius: 0 !important;
}
.ta::placeholder {
  color: var(--ui-text-3);
}
.ta:disabled {
  opacity: 0.55;
}
/* 焦点提示交给外层容器的 focus-within，避免框内再套一圈环 */
.ta:focus-visible {
  box-shadow: none;
}

/* 底栏 */
.foot {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding-top: var(--ui-sp-2);
}
.foot-gap {
  flex: 1;
}
.tool {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  height: 26px;
  padding: 0 var(--ui-sp-2);
  border: none;
  border-radius: var(--ui-r-pill);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.tool:hover:not(:disabled) {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.tool:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

/* Agent 模式：胶囊开关，选中用 accent 浅底 + accent 文字 */
.agent {
  height: 26px;
  padding: 0 var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-pill);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  white-space: nowrap;
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease),
    border-color var(--ui-dur-1) var(--ui-ease);
}
.agent:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.agent.on {
  background: var(--ui-accent-soft);
  border-color: var(--ui-accent-line);
  color: var(--ui-accent);
}

/* 停止态：用 --ui-err 而不是主色（覆盖 AppButton 内部样式） */
.foot :deep(.app-btn.stop-btn) {
  background: color-mix(in srgb, var(--ui-err) 16%, transparent);
  border-color: var(--ui-err);
  color: var(--ui-err);
}
.foot :deep(.app-btn.stop-btn:hover:not(:disabled)) {
  background: color-mix(in srgb, var(--ui-err) 26%, transparent);
}

/* 附件面板 */
.attach-panel {
  padding-bottom: var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
.attach-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
}
.attach-tag {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-2);
  max-width: 100%;
  padding: var(--ui-sp-1) var(--ui-sp-2);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  overflow-wrap: anywhere;
}
.attach-name {
  min-width: 0;
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
}
.attach-del {
  flex: none;
  border: none;
  background: transparent;
  color: var(--ui-text-3);
  font-size: var(--ui-fs-12);
  white-space: nowrap;
  transition: color var(--ui-dur-1) var(--ui-ease);
}
.attach-del:hover:not(:disabled) {
  color: var(--ui-err);
}
.attach-note {
  margin: var(--ui-sp-1) 0 0;
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
.attach-error {
  margin: var(--ui-sp-1) 0 0;
  color: var(--ui-err);
  line-height: var(--ui-line);
}
</style>
