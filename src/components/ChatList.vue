<!-- 《铃·记忆体》消息列表：遍历 chatStore.messages，渲染 ChatBubble
     任务 3：自动滚动到底部、流式期间保持跟随 -->
<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import ChatBubble from './ChatBubble.vue'
import { useChatStore } from '../stores/chatStore'

const chat = useChatStore()
const scrollRef = ref<HTMLElement | null>(null)

// 滚动到底部辅助函数
async function scrollToBottom() {
  await nextTick()
  if (scrollRef.value) {
    scrollRef.value.scrollTop = scrollRef.value.scrollHeight
  }
}

// 新消息加入 / 内容变化时滚动到底部
watch(
  () => [chat.messages, chat.streamingId],
  () => scrollToBottom(),
  { deep: true }
)
</script>

<template>
  <div ref="scrollRef" class="chat-list">
    <div v-if="chat.messages.length === 0" class="empty-hint">
      <p>和铃说点什么吧～</p>
    </div>
    <template v-for="msg in chat.messages" :key="msg.id">
      <ChatBubble :message="msg" />
    </template>
  </div>
</template>

<style scoped>
.chat-list {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px;
  display: flex;
  flex-direction: column;
  scrollbar-width: thin;
}
.empty-hint {
  margin: auto;
  text-align: center;
  color: var(--text-secondary, #aaa);
}
.empty-hint .paw {
  font-size: 40px;
  display: block;
  margin-bottom: 8px;
}
</style>
