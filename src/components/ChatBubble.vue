<!-- 《铃·记忆体》对话消息：铃（头像 + 正文 + 时间，无气泡底）/ 主人（右对齐，raised 底 + 大圆角）
     保留流式光标、生成中断提示、子任务折叠卡、附件行 -->
<script setup lang="ts">
import { computed } from 'vue'
import type { Message } from '../types'
import { useChatStore } from '../stores/chatStore'
import { useSettingStore } from '../stores/settingStore'
import { assetUrl, isImagePath } from '../utils/tauri'
import SearchModePanel from './SearchModePanel.vue'
import suzuAvatar from '../assets/avatar_suzu.png'

const props = defineProps<{ message: Message; isLast?: boolean }>()
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
// 铃的头像：用户自定义图片则显示，否则用内置猫娘头像
const avatarImg = computed(() => (isImagePath(setting.avatarSuzu) ? assetUrl(setting.avatarSuzu!) : suzuAvatar))
// 用户头像：图片路径则显示图片，否则文字占位
const userAvatarImg = computed(() => (isImagePath(setting.avatarUser) ? assetUrl(setting.avatarUser!) : null))

// 显示文本：聊天气泡不渲染 Markdown，裸的星号/井号很丑
// 这里把常见 Markdown 标记「降级」成纯文本（去掉符号、保留内容），
// 这样即使模型偶尔输出了 **加粗** 或 ### 标题，显示出来也是干净的。
const displayContent = computed(() => {
  const raw = props.message.content ?? ''
  if (!raw) return ''
  return (
    raw
      // 代码块围栏（```lang ... ```）→ 只留内容
      .replace(/```[^\n]*\n?([\s\S]*?)```/g, '$1')
      // 行内代码 `code` → code
      .replace(/`([^`\n]+)`/g, '$1')
      // **粗体** / __粗体__ → 粗体
      .replace(/\*\*([^*\n]+)\*\*/g, '$1')
      .replace(/__([^_\n]+)__/g, '$1')
      // *斜体* / _斜体_ → 斜体（避免误伤 URL 和乘号，要求两侧非空白且不与其他星号相连）
      .replace(/(^|[^*\w])\*(\S(?:[^*\n]*?\S)?)\*(?!\*)/g, '$1$2')
      .replace(/(^|[^_\w])_(\S(?:[^_\n]*?\S)?)_(?!_)/g, '$1$2')
      // 行首标题 ### → 去掉井号
      .replace(/^#{1,6}\s+/gm, '')
      // 行首引用 > → 去掉
      .replace(/^>\s?/gm, '')
      // 无序列表标记 "- " / "* " / "+ " 保留缩进但去掉符号（保留换行结构）
      .replace(/^[ \t]*[-*+]\s+/gm, '· ')
  )
})
</script>

<template>
  <div class="row" :class="isUser ? 'row-user' : 'row-suzu'">
    <!-- 铃的头像 -->
    <div v-if="!isUser" class="avatar">
      <img v-if="avatarImg" :src="avatarImg" class="avatar-img" alt="铃" />
      <span v-else class="avatar-text">{{ setting.avatarSuzu || '铃' }}</span>
    </div>

    <div class="msg" :class="isUser ? 'msg-user' : 'msg-suzu'">
      <span class="content">{{ displayContent }}</span>

      <!-- 子任务折叠卡 -->
      <details v-for="result in chat.subAgentResults[message.id]" :key="result.sub_id" class="sub-agent">
        <summary>{{ result.goal }} · {{ result.status }}</summary>
        <p class="sub-agent-body">{{ result.summary || '正在处理子任务…' }}</p>
      </details>

      <!-- 附件行 -->
      <p v-for="(attachment, index) in message.attachments" :key="index" class="attach">
        [{{ attachment.kind === 'image' ? '图片' : '附件' }}: {{ attachment.name }}]
      </p>

      <!-- 流式光标：2px 竖线闪烁 -->
      <span v-if="isStreaming" class="cursor" aria-hidden="true" />

      <!-- 生成中断提示 -->
      <p v-if="isInterrupted" class="interrupt">生成中断，请重试</p>

      <time class="time">{{ time }}</time>

      <!-- 离线增强方案入口：仅在离线模式最后一条铃消息下显示 -->
      <SearchModePanel v-if="!isUser && isLast && setting.modelMode === 'script'" />
    </div>

    <!-- 主人头像 -->
    <div v-if="isUser" class="avatar avatar-user">
      <img v-if="userAvatarImg" :src="userAvatarImg" class="avatar-img" alt="主人" />
      <span v-else class="avatar-text">{{ setting.avatarUser || '我' }}</span>
    </div>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  align-items: flex-start;
  gap: var(--ui-sp-2);
  width: 100%;
  min-width: 0;
}
.row-user {
  justify-content: flex-end;
}

/* 头像：26px 圆形 */
.avatar {
  width: 26px;
  height: 26px;
  margin-top: var(--ui-sp-1);
  flex: none;
  display: grid;
  place-items: center;
  overflow: hidden;
  border-radius: var(--ui-r-pill);
  background: var(--ui-hover);
}
.avatar-user {
  background: var(--ui-accent-soft);
}
.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.avatar-text {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: 1;
  user-select: none;
}
.avatar-user .avatar-text {
  color: var(--ui-accent);
}

/* 消息主体：铃默认无底色（--ui-bubble-suzu 是 transparent），客人自己设了气泡色才显色 */
.msg {
  max-width: 72%;
  min-width: 0;
  font-size: var(--ui-fs-14);
  line-height: var(--ui-line);
  color: var(--ui-text);
  padding-top: var(--ui-sp-1);
}
.msg-suzu {
  padding: var(--ui-sp-2) var(--ui-sp-3);
  background: var(--ui-bubble-suzu);
  border-radius: var(--ui-r-lg);
}
.msg-user {
  padding: var(--ui-sp-2) var(--ui-sp-3);
  background: var(--ui-bubble-user);
  border-radius: var(--ui-r-lg);
}

/* 正文：允许换行 */
.content {
  display: inline;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

/* 子任务折叠卡 */
.sub-agent {
  margin-top: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
.sub-agent summary {
  cursor: pointer;
  overflow-wrap: anywhere;
}
.sub-agent-body {
  margin: var(--ui-sp-2) 0 0;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  line-height: var(--ui-line);
}

/* 附件行 */
.attach {
  margin: var(--ui-sp-1) 0 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  overflow-wrap: anywhere;
  line-height: var(--ui-line);
}

/* 流式光标：2px 竖线闪烁 */
.cursor {
  display: inline-block;
  width: 2px;
  height: 1em;
  margin-left: var(--ui-sp-1);
  vertical-align: text-bottom;
  background: var(--ui-accent);
  animation: cursor-blink 1s steps(2, start) infinite;
}
@keyframes cursor-blink {
  to {
    opacity: 0;
  }
}

/* 生成中断 */
.interrupt {
  margin: var(--ui-sp-1) 0 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-err);
  line-height: var(--ui-line);
}

/* 时间 */
.time {
  display: block;
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: 1;
}
.msg-user .time {
  text-align: right;
}
</style>
