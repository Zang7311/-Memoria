<!-- 《铃·记忆体》对话气泡：用户（右/深色）/ 铃（左/浅渐变+猫爪）
     任务 2：支持流式光标闪烁、中断提示、时间戳 -->
<script setup lang="ts">
import { computed } from 'vue'
import type { Message } from '../types'
import { useChatStore } from '../stores/chatStore'
import { useSettingStore } from '../stores/settingStore'

const props = defineProps<{ message: Message }>()
const chat = useChatStore()
const setting = useSettingStore()

// 是否正在流式输出（光标闪烁条件）
const isStreaming = computed(() => chat.streamingId === props.message.id)
// 是否被中断/出错
const isInterrupted = computed(() => !!chat.interruptedIds[props.message.id])
// 时间戳格式化为 HH:mm
const time = computed(() => {
  const d = new Date(props.message.timestamp)
  if (Number.isNaN(d.getTime())) return ''
  const h = String(d.getHours()).padStart(2, '0')
  const m = String(d.getMinutes()).padStart(2, '0')
  return `${h}:${m}`
})
const isUser = computed(() => props.message.role === 'user')
</script>

<template>
  <div class="bubble-row" :class="isUser ? 'row-user' : 'row-suzu'">
    <!-- 铃的头像（猫娘） -->
    <div v-if="!isUser" class="avatar">{{ setting.avatarSuzu || '铃' }}</div>

    <div class="bubble" :class="isUser ? 'bubble-user' : 'bubble-suzu'">
      <span class="content">{{ message.content }}</span>
      <!-- 流式光标：闪烁 -->
      <span v-if="isStreaming" class="cursor">▍</span>
      <!-- 中断提示 -->
      <span v-if="isInterrupted" class="error-tip">（生成中断，请重试）</span>
      <div class="time">{{ time }}</div>
    </div>

    <div v-if="isUser" class="avatar avatar-user">🧑</div>
  </div>
</template>

<style scoped>
.bubble-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin: 12px 0;
  width: 100%;
}
.row-user {
  justify-content: flex-end;
}
.row-suzu {
  justify-content: flex-start;
}
.avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: linear-gradient(135deg, #ffe4e1, #fff0f5);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  flex-shrink: 0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
}
.avatar-user {
  background: var(--bubble-user-bg, #2d2d2d);
}
.bubble {
  max-width: 72%;
  padding: 10px 14px;
  border-radius: 16px;
  font-size: 14px;
  line-height: 1.6;
  position: relative;
  word-break: break-word;
  white-space: pre-wrap;
}
.bubble-user {
  background: var(--bubble-user-bg, #2d2d2d);
  color: var(--text-user, #fff);
  border-bottom-right-radius: 4px;
}
.bubble-suzu {
  background: var(--bubble-suzu-bg, linear-gradient(135deg, #ffe4e1, #fff0f5));
  color: var(--text-suzu, #5b3a63);
  border-bottom-left-radius: 4px;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
}
.content {
  white-space: pre-wrap;
}
/* 闪烁光标 */
.cursor {
  display: inline-block;
  margin-left: 2px;
  color: inherit;
  animation: blink 0.9s steps(2, start) infinite;
}
@keyframes blink {
  to {
    visibility: hidden;
  }
}
.error-tip {
  display: block;
  margin-top: 4px;
  font-size: 12px;
  color: var(--danger, #d9534f);
  opacity: 0.85;
}
.time {
  margin-top: 6px;
  font-size: 11px;
  text-align: right;
  opacity: 0.55;
}
</style>
