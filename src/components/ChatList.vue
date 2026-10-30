<!-- 《铃·记忆体》消息列表：遍历 chatStore.messages 渲染 ChatBubble
     内容区纵向滚动、宽度自适应（不设固定宽度）；新消息/流式期间保持贴底 -->
<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import ChatBubble from './ChatBubble.vue'
import { useChatStore } from '../stores/chatStore'

const chat = useChatStore()
const scrollRef = ref<HTMLElement | null>(null)

// 找到真正能滚的容器：本层没被撑满时向上找（外层对话流也可能在滚）
function findScroller(start: HTMLElement | null): HTMLElement | null {
  let el: HTMLElement | null = start
  for (let i = 0; el && i < 4; i += 1) {
    const overflowY = getComputedStyle(el).overflowY
    if ((overflowY === 'auto' || overflowY === 'scroll') && el.scrollHeight > el.clientHeight) return el
    el = el.parentElement
  }
  return start
}

// 滚动到底部辅助函数
async function scrollToBottom() {
  await nextTick()
  const el = findScroller(scrollRef.value)
  if (el) el.scrollTop = el.scrollHeight
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
    <div v-if="chat.messages.length === 0" class="empty">
      <p class="empty-slogan">我，与你交谈，为你存忆</p>
      <p class="empty-text">你好，我是铃～可以陪你聊天，也可以帮你打理电脑。</p>
      <p class="empty-tip">试试对我说「你好」或「铃，清理一下内存」</p>
    </div>
    <ChatBubble
      v-for="(msg, i) in chat.messages"
      :key="msg.id"
      :message="msg"
      :isLast="i === chat.messages.length - 1"
    />
  </div>
</template>

<style scoped>
.chat-list {
  flex: 1;
  min-height: 0;
  width: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-4);
  padding-bottom: var(--ui-sp-4);
  scrollbar-width: thin;
}
.empty {
  margin: auto;
  padding: var(--ui-sp-6) var(--ui-sp-4);
  text-align: center;
}
.empty-slogan {
  margin: 0 0 var(--ui-sp-3);
  font-size: var(--ui-fs-20);
  font-weight: 600;
  color: var(--ui-accent);
  letter-spacing: 2px;
}
.empty-text {
  margin: 0 0 var(--ui-sp-2);
  font-size: var(--ui-fs-13);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
}
.empty-tip {
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
</style>
