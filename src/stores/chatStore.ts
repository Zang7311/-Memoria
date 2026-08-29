// 《铃·记忆体》对话 Store（useChatStore）
// 任务书 3.3 + 任务 5：管理消息列表、加载状态、输入框、流式渲染相关状态
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Message } from '../types'

export const useChatStore = defineStore('chat', () => {
  // 对话消息列表
  const messages = ref<Message[]>([])
  // 是否正在加载（等待 AI 回复 / 流式接收中）
  const isLoading = ref(false)
  // 当前输入框内容
  const inputText = ref('')
  // 正在流式输出的消息 id（用于气泡闪烁光标；无则 null）
  const streamingId = ref<string | null>(null)
  // 被中断/出错的消息 id 集合（用于显示“生成中断”提示）
  const interruptedIds = ref<Record<string, boolean>>({})

  // 基础 action：追加一条消息
  function addMessage(msg: Message) {
    messages.value.push(msg)
  }

  // 基础 action：清空对话
  function clearMessages() {
    messages.value = []
    streamingId.value = null
    interruptedIds.value = {}
  }

  // 流式开始：标记当前正在输出的消息并上锁
  function beginStream(id: string) {
    streamingId.value = id
    isLoading.value = true
  }

  // 流式追加：把收到的片段 append 到指定消息
  function appendToMessage(id: string, chunk: string) {
    const msg = messages.value.find((m) => m.id === id)
    if (msg) msg.content += chunk
  }

  // 流式正常结束：关闭光标、解锁
  function finishStream(id: string) {
    if (streamingId.value === id) streamingId.value = null
    isLoading.value = false
  }

  // 流式出错：标记中断提示、关闭光标、解锁
  function errorStream(id: string) {
    interruptedIds.value[id] = true
    if (streamingId.value === id) streamingId.value = null
    isLoading.value = false
  }

  return {
    messages,
    isLoading,
    inputText,
    streamingId,
    interruptedIds,
    addMessage,
    clearMessages,
    beginStream,
    appendToMessage,
    finishStream,
    errorStream,
  }
})
