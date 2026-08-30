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
      <p class="empty-slogan">我，与你交谈，为你存忆</p>
      <p class="empty-sub">你好，我是铃～可以陪你聊天，也可以帮你打理电脑。</p>
      <p class="empty-tip">试试对我说「你好」或「铃，清理一下内存」</p>
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
.empty-slogan {
  font-size: var(--fs-20);
  font-weight: 600;
  color: var(--accent, #ff7a94);
  letter-spacing: 2px;
  margin-bottom: 10px;
}
.empty-sub {
  font-size: var(--fs-13);
  color: var(--text-secondary, #aaa);
  margin-bottom: 8px;
}
.empty-tip {
  font-size: var(--fs-12);
  color: var(--text-secondary, #888);
  opacity: 0.8;
}
.empty-hint .paw {
  font-size: var(--fs-40);
  display: block;
  margin-bottom: 8px;
}
</style>
