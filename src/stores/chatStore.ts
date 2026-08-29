// 《铃·记忆体》对话 Store（useChatStore）— 收尾工程师批次3：多会话重构
// 支持：多会话标签（新建/切换/删除）、会话持久化（后端 sessions/）、流式渲染。
// 每个会话有独立的 messages 列表，切换/结束时自动保存到后端。
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ChatUsage, Message, SessionMeta } from '../types'
import {
  createSession as createSessionCmd,
  deleteSession as deleteSessionCmd,
  listSessions,
  loadSession as loadSessionCmd,
  saveSession as saveSessionCmd,
} from '../utils/tauri'

export const useChatStore = defineStore('chat', () => {
  // 会话列表 + 当前活跃会话
  const sessions = ref<SessionMeta[]>([])
  const activeSessionId = ref<string | null>(null)
  // 当前活跃会话的消息流
  const messages = ref<Message[]>([])
  // 是否正在加载（等待 AI 回复 / 流式接收中）
  const isLoading = ref(false)
  // 当前输入框内容
  const inputText = ref('')
  // 正在流式输出的消息 id（用于气泡闪烁光标；无则 null）
  const streamingId = ref<string | null>(null)
  // 被中断/出错的消息 id 集合
  const interruptedIds = ref<Record<string, boolean>>({})
  // 最近一次 API 回复的 token 用量
  const lastUsage = ref<ChatUsage | null>(null)

  // —— 初始化：加载会话列表，无会话则自动新建 ——
  async function init() {
    try {
      sessions.value = await listSessions()
    } catch {
      sessions.value = []
    }
    if (sessions.value.length === 0) {
      await createSession()
    } else if (!activeSessionId.value) {
      activeSessionId.value = sessions.value[0].id
      await loadInto(sessions.value[0].id)
    }
  }

  // 把指定会话的消息加载进内存
  async function loadInto(id: string) {
    const s = await loadSessionCmd(id)
    messages.value = s.messages
    streamingId.value = null
    interruptedIds.value = {}
  }

  // —— 新建会话 ——
  async function createSession() {
    await saveCurrentSession().catch(() => {})
    const s = await createSessionCmd()
    sessions.value.unshift(s.meta)
    activeSessionId.value = s.meta.id
    messages.value = []
    streamingId.value = null
    interruptedIds.value = {}
    return s
  }

  // —— 切换会话 ——
  async function switchSession(id: string) {
    if (id === activeSessionId.value) return
    await saveCurrentSession().catch(() => {})
    activeSessionId.value = id
    await loadInto(id)
  }

  // —— 保存当前会话到后端（更新标题/计数/时间）——
  async function saveCurrentSession() {
    if (!activeSessionId.value) return
    const s = await saveSessionCmd(activeSessionId.value, messages.value)
    const idx = sessions.value.findIndex((x) => x.id === s.meta.id)
    if (idx >= 0) sessions.value[idx] = s.meta
    else sessions.value.unshift(s.meta)
    // 最新会话排最前
    sessions.value.sort((a, b) => (a.updated_at < b.updated_at ? 1 : -1))
  }

  // —— 删除会话（若删除当前会话则切换到下一个/新建）——
  async function deleteSession(id: string) {
    await deleteSessionCmd(id)
    sessions.value = sessions.value.filter((x) => x.id !== id)
    if (activeSessionId.value === id) {
      activeSessionId.value = null
      messages.value = []
      if (sessions.value.length > 0) {
        await switchSession(sessions.value[0].id)
      } else {
        await createSession()
      }
    }
  }

  // —— 基础消息 action（作用于当前会话 messages）——
  function addMessage(msg: Message) {
    messages.value.push(msg)
  }

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

  // 流式正常结束
  function finishStream(id: string) {
    if (streamingId.value === id) streamingId.value = null
    isLoading.value = false
  }

  // 流式出错：标记中断提示
  function errorStream(id: string) {
    interruptedIds.value[id] = true
    if (streamingId.value === id) streamingId.value = null
    isLoading.value = false
  }

  // 记录最近一次 token 用量（API 模式）
  function setUsage(u: ChatUsage | null) {
    lastUsage.value = u
  }

  return {
    sessions,
    activeSessionId,
    messages,
    isLoading,
    inputText,
    streamingId,
    interruptedIds,
    lastUsage,
    init,
    createSession,
    switchSession,
    deleteSession,
    saveCurrentSession,
    addMessage,
    clearMessages,
    beginStream,
    appendToMessage,
    finishStream,
    errorStream,
    setUsage,
  }
})
