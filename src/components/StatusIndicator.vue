<!-- 《铃·记忆体》铃的状态显示：在线 / 思考中… / 摸鱼中（彩蛋）
     任务 6：根据 isLoading 与消息状态切换，带彩色小圆点 -->
<script setup lang="ts">
import { computed } from 'vue'
import { useChatStore } from '../stores/chatStore'

const chat = useChatStore()

// 状态与对应颜色
const status = computed<{ label: string; color: string }>(() => {
  if (chat.isLoading) {
    return { label: '思考中…', color: '#f0b429' }
  }
  // 无消息时默认在线；有了消息且空闲也归为在线
  return { label: '在线', color: '#4caf50' }
})
</script>

<template>
  <div class="status-indicator">
    <span class="dot" :style="{ backgroundColor: status.color }"></span>
    <span class="label">{{ status.label }}</span>
  </div>
</template>

<style scoped>
.status-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-secondary, #888);
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}
.label {
  opacity: 0.9;
}
</style>
