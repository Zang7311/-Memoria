<!-- 《铃·记忆体》输入栏：textarea + 纸飞机发送按钮
     任务 4：Enter 发送、Shift+Enter 换行、发送中禁用、清空输入 -->
<script setup lang="ts">
import { useChatStore } from '../stores/chatStore'
import { useStreamRender } from '../composables/useStreamRender'
import { useSettingStore } from '../stores/settingStore'

const chat = useChatStore()
const setting = useSettingStore()
const { send } = useStreamRender()

// 处理 Enter 键：Shift+Enter 换行，Enter 发送
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
}

// 发送逻辑：携带设置里的思考深度（文库/API/本地模式均生效）
async function handleSend() {
  const content = chat.inputText.trim()
  if (!content || chat.isLoading) return
  await send(content, setting.depth)
  chat.inputText = ''
}
</script>

<template>
  <div class="chat-input">
    <textarea
      v-model="chat.inputText"
      class="input-area"
      name="chat"
      placeholder="说点什么…"
      :disabled="chat.isLoading"
      rows="1"
      @keydown="onKeydown"
    ></textarea>
    <button
      class="send-btn"
      :disabled="chat.isLoading || !chat.inputText.trim()"
      title="发送"
      @click="handleSend"
    >
      ✈️
    </button>
  </div>
</template>

<style scoped>
.chat-input {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 8px 16px 22px;
  border-top: 1px solid var(--border, rgba(128, 128, 128, 0.2));
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
  font-size: 14px;
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
  color: #fff;
  font-size: 20px;
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
</style>
