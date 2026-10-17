<!-- 《铃·记忆体》桌宠 v8（悬浮球功能打回重做）
     定位：桌面宠物，而不是一个圆球。
       - 形象：头像（默认，稳定）/ 纯文字 / Live2D（实验，高缩放屏可能被裁）
       - 生命感：待机小动作、偶尔自言自语、5 分钟没人理会趴下打瞌睡
       - 互动：单击戳一下、长按摸摸头、拖动拎起来、双击快速提问
       - 右键：控制台（打开主窗口 / 换形象 / 穿透 / 监测 / 设置 / 退出）
       - 反馈：头顶台词气泡（点击、摸头、自言自语、屏幕监测提醒）
     窗口：透明置顶小窗（lib.rs 创建，启动随配置常驻）。
     ⚠️ 位置只由用户用系统拖动决定；本组件刻意不程序化移动窗口 ——
        150% 缩放下移动透明窗口会让 WebView 画面与外壳脱节（实机踩到）。 -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { emit, listen } from '@tauri-apps/api/event'
import {
  executeToolbox,
  getFloatingBallClickThrough,
  getMonitorRules,
  listSessions,
  listToolboxItems,
  onBallClickThroughChanged,
  onChatChunk,
  onChatEnd,
  onChatError,
  onConfigUpdated,
  onMonitorTrigger,
  quitApp,
  sendMessage,
  setFloatingBallClickThrough,
  toggleMonitoring,
} from '../utils/tauri'
import { useSettingStore } from '../stores/settingStore'
import {
  BUBBLE_MS,
  SLEEP_AFTER_MS,
  nextChatterDelay,
  nextIdleDelay,
  pickPetLine,
} from '../utils/pet'
import ballAvatar from '../assets/ball_avatar.png'

const win = getCurrentWindow()
const setting = useSettingStore()

const mode = computed(() => setting.floatingBallMode)
const enabled = computed(() => setting.floatingBallEnabled)
const size = computed(() => setting.floatingBallSize)
const opacity = computed(() => setting.floatingBallOpacity)
const breathing = computed(() => setting.floatingBallBreathing)

// ==================== 桌宠状态 ====================
type PetState = 'idle' | 'sleep' | 'poke' | 'pat' | 'drag' | 'fall' | 'hop'
const petState = ref<PetState>('idle')
const zzz = ref(false)
const bubble = ref('')
const bubbleOn = ref(false)
const clickThrough = ref(false)
const monitorOn = ref(true)
const hasMessage = ref(false)

// 窗口尺寸：角色区 + 顶部台词气泡区
const BUBBLE_H = 64
const roleHeight = computed(() => (mode.value === 'live2d' ? 400 : size.value))
const winW = computed(() => (mode.value === 'live2d' ? 300 : size.value))
const winH = computed(() => roleHeight.value + BUBBLE_H)

const stageClass = computed(() => [`state-${petState.value}`, { breathing: breathing.value && petState.value === 'idle', dim: petState.value === 'sleep' }])

// ==================== 常量 ====================
const DRAG_THRESHOLD = 10 // 拖动判定阈值（px）
const CLICK_WINDOW = 280 // 双击判定间隔（ms）
const PAT_HOLD_MS = 700 // 长按摸头时长
const PANEL_W = 232 // 控制台宽
const PANEL_H = 344 // 控制台高
const ASK_W = 300 // 快速提问卡宽
const ASK_H = 320 // 快速提问卡高

// ==================== 拖拽（系统级拖动，OS 接管） ====================
let dragging = false
let moved = false
let osDragging = false
let pressX = 0
let pressY = 0
let longTimer: number | undefined
let clickTimer: number | undefined
let lastClickAt = 0
let bubbleTimer: number | undefined

// ==================== 鼠标事件 ====================
let lastActive = Date.now()

function touch() {
  lastActive = Date.now()
  if (zzz.value) {
    zzz.value = false
    petState.value = 'idle'
    say(pickPetLine('wake'))
  }
}

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  // 展开态里所有点击都归卡片自己处理（否则输入框会被 preventDefault 抢掉焦点）
  if (panelOpen.value || askOpen.value) return
  touch()
  e.preventDefault()
  dragging = true
  moved = false
  osDragging = false
  pressX = e.screenX
  pressY = e.screenY
  if (longTimer) clearTimeout(longTimer)
  longTimer = window.setTimeout(() => {
    if (dragging && !moved && !osDragging && !panelOpen.value) playPat()
  }, PAT_HOLD_MS)
}

function onMouseMove(e: MouseEvent) {
  if (!dragging || osDragging || moved) return
  const dx = e.screenX - pressX
  const dy = e.screenY - pressY
  if (Math.hypot(dx, dy) < DRAG_THRESHOLD) return
  moved = true
  if (longTimer) {
    clearTimeout(longTimer)
    longTimer = undefined
  }
  void startOsDrag()
}

async function startOsDrag() {
  if (osDragging) return
  osDragging = true
  petState.value = 'drag'
  say(pickPetLine('drag'))
  try {
    await win.startDragging()
  } catch {
    /* 忽略 */
  }
  osDragging = false
  dragging = false
  if (longTimer) {
    clearTimeout(longTimer)
    longTimer = undefined
  }
  await dropAfterDrag()
}

function onMouseUp() {
  if (osDragging) return
  if (!dragging) return
  dragging = false
  if (longTimer) {
    clearTimeout(longTimer)
    longTimer = undefined
  }
  if (!moved) handleClick()
}

/** 单击戳一下 / 双击快速提问 */
function handleClick() {
  const now = Date.now()
  if (now - lastClickAt < CLICK_WINDOW) {
    if (clickTimer) {
      clearTimeout(clickTimer)
      clickTimer = undefined
    }
    lastClickAt = 0
    void quickAsk()
    return
  }
  lastClickAt = now
  if (clickTimer) clearTimeout(clickTimer)
  clickTimer = window.setTimeout(() => {
    clickTimer = undefined
    lastClickAt = 0
    playPoke()
  }, CLICK_WINDOW + 20)
}

// ==================== 桌宠动作 ====================
let actionTimer: number | undefined

function backToIdle(delay: number) {
  if (actionTimer) clearTimeout(actionTimer)
  actionTimer = window.setTimeout(() => {
    if (petState.value !== 'drag' && petState.value !== 'fall') petState.value = 'idle'
  }, delay)
}

function playPoke() {
  if (zzz.value) return
  petState.value = 'poke'
  say(pickPetLine('poke'))
  backToIdle(420)
}

function playPat() {
  if (zzz.value) {
    touch()
    return
  }
  petState.value = 'pat'
  say(pickPetLine('pat'))
  backToIdle(950)
}

function playHop() {
  if (petState.value !== 'idle' || panelOpen.value || askOpen.value || clickThrough.value || zzz.value) return
  petState.value = 'hop'
  backToIdle(700)
}

function say(text: string) {
  if (!text) return
  bubble.value = text
  bubbleOn.value = true
  if (bubbleTimer) clearTimeout(bubbleTimer)
  bubbleTimer = window.setTimeout(() => {
    bubbleOn.value = false
  }, BUBBLE_MS)
}

// ==================== 松手之后 ====================
// 说明：这里刻意不做「掉到屏幕底」的窗口位移 —— 在 150% 缩放 + 透明窗口上，
// 程序化移动窗口会让 WebView 画面跟外壳脱节（实机踩到，画面跑到屏幕外）。
// 桌宠的位置只由用户用系统拖动决定，系统拖动时画面能正确跟随。
async function dropAfterDrag() {
  petState.value = 'idle'
  if (Math.random() < 0.5) say(pickPetLine('drop'))
}

// ==================== 右键控制台 ====================
const panelOpen = ref(false)
const askOpen = ref(false)
const panelPage = ref<'main' | 'tools'>('main')
const askText = ref('')
const askInput = ref<HTMLInputElement | null>(null)
const askMsgs = ref<{ role: 'user' | 'suzu'; text: string }[]>([])
const askStreaming = ref('')
const askBusy = ref(false)
const askComposing = ref(false)
let askUnlisten: (() => void)[] = []

// 展开/收起只改尺寸、不动位置：移动透明窗口会让 WebView 画面与外壳脱节。
// 展开时窗口以左上角为锚点变大，收起再缩回，桌宠始终待在原处。
async function expandTo(w: number, h: number, kind: 'panel' | 'ask') {
  if (kind === 'ask' && panelOpen.value) panelOpen.value = false
  if (kind === 'panel' && askOpen.value) askOpen.value = false
  if (kind === 'ask') askOpen.value = true
  if (kind === 'panel') panelOpen.value = true
  if (mode.value === 'live2d' && live2dCleanup) live2dCleanup()
  try {
    await win.setSize(new LogicalSize(w, h))
  } catch {
    /* 忽略 */
  }
}

async function restoreBall() {
  try {
    await win.setSize(new LogicalSize(winW.value, winH.value))
  } catch {
    /* 忽略 */
  }
  if (mode.value === 'live2d') {
    nextTick(() => loadLive2D())
  }
}

async function openPanel() {
  if (panelOpen.value) return
  panelPage.value = 'main'
  await expandTo(PANEL_W, PANEL_H, 'panel')
  try {
    const res = await getMonitorRules()
    monitorOn.value = res.enabled
  } catch {
    /* 忽略 */
  }
}

async function closePanel() {
  if (!panelOpen.value) return
  panelOpen.value = false
  await restoreBall()
}

// ==================== 快速提问卡 ====================
async function openAsk() {
  if (askOpen.value) return
  askText.value = ''
  await expandTo(ASK_W, ASK_H, 'ask')
  try {
    askUnlisten = [
      await onChatChunk((chunk) => {
        if (!askOpen.value) return
        askStreaming.value += chunk
      }),
      await onChatEnd(() => {
        if (!askOpen.value) return
        const full = askStreaming.value.trim()
        askStreaming.value = ''
        askBusy.value = false
        if (full) askMsgs.value.push({ role: 'suzu', text: full })
        scrollAskToBottom()
      }),
      await onChatError((err) => {
        if (!askOpen.value) return
        askBusy.value = false
        askStreaming.value = ''
        askMsgs.value.push({ role: 'suzu', text: `（铃回复失败：${err}）` })
        scrollAskToBottom()
      }),
    ]
  } catch {
    /* 忽略 */
  }
  setTimeout(() => askInput.value?.focus(), 180)
}

async function closeAsk() {
  if (!askOpen.value) return
  askOpen.value = false
  askBusy.value = false
  askStreaming.value = ''
  askUnlisten.forEach((fn) => {
    try {
      fn()
    } catch {
      /* 忽略 */
    }
  })
  askUnlisten = []
  await restoreBall()
}

const askBodyRef = ref<HTMLDivElement | null>(null)
function scrollAskToBottom() {
  setTimeout(() => {
    if (askBodyRef.value) askBodyRef.value.scrollTop = askBodyRef.value.scrollHeight
  }, 30)
}

const MAX_ASK_INPUT = 4000
function onAskEnter() {
  if (askComposing.value) return
  void submitAsk()
}
async function submitAsk() {
  const text = askText.value.trim()
  if (!text || askBusy.value) return
  if (text.length > MAX_ASK_INPUT) {
    showNotice(`内容过长（${text.length}/${MAX_ASK_INPUT}字）`)
    return
  }
  askText.value = ''
  askMsgs.value.push({ role: 'user', text })
  scrollAskToBottom()
  askBusy.value = true
  askStreaming.value = ''
  let sid: string | null = null
  try {
    const sessions = await listSessions()
    if (sessions && sessions.length > 0) sid = sessions[0].id
  } catch {
    /* 默认会话 */
  }
  try {
    await sendMessage(text, setting.depth || 2, sid)
  } catch {
    askBusy.value = false
    askMsgs.value.push({ role: 'suzu', text: '（发送失败，请检查连接）' })
    scrollAskToBottom()
  }
}

// ==================== 主窗口 / 设置 ====================
let fireToMainTimers: number[] = []
function fireToMain(eventName: string) {
  fireToMainTimers.forEach(clearTimeout)
  fireToMainTimers = []
  for (const d of [120, 450, 850, 1300, 1900]) {
    fireToMainTimers.push(window.setTimeout(() => { emit(eventName).catch(() => {}) }, d))
  }
}

async function showMain() {
  try {
    const main = await WebviewWindow.getByLabel('main').catch(() => null)
    if (main) {
      await main.show().catch(() => {})
      await main.unminimize().catch(() => {})
      await main.setFocus().catch(() => {})
    } else {
      showNotice('请通过系统托盘图标打开主窗口')
    }
  } catch {
    showNotice('打开主窗口失败')
  }
}

async function openSettings() {
  await showMain()
  fireToMain('floating-open-settings')
}

// ==================== 提示条 ====================
const notice = ref('')
let noticeTimer: number | undefined
function showNotice(text: string) {
  notice.value = text
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = window.setTimeout(() => (notice.value = ''), 2600)
}

// ==================== 定时行为 ====================
let idleTimer: number | undefined
let chatterTimer: number | undefined
let watchdog: number | undefined

function scheduleIdleAction() {
  idleTimer = window.setTimeout(() => {
    playHop()
    scheduleIdleAction()
  }, nextIdleDelay())
}

function scheduleChatter() {
  chatterTimer = window.setTimeout(() => {
    maybeChatter()
    scheduleChatter()
  }, nextChatterDelay())
}

function maybeChatter() {
  if (!setting.floatingBallChatter) return
  if (petState.value !== 'idle' || panelOpen.value || askOpen.value || clickThrough.value || zzz.value) return
  say(pickPetLine('chatter'))
}

function maybeSleep() {
  if (petState.value !== 'idle' || panelOpen.value || askOpen.value || zzz.value) return
  if (Date.now() - lastActive < SLEEP_AFTER_MS) return
  zzz.value = true
  petState.value = 'sleep'
  say(pickPetLine('sleep'))
}

// ==================== 窗口事件 ====================
let unlistenTrigger: (() => void) | undefined
let unlistenCfg: (() => void) | undefined
let unlistenCt: (() => void) | undefined
let unlistenFocus: (() => void) | undefined
let unlistenToggle: (() => void) | undefined
let unlistenAskHotkey: (() => void) | undefined
let sizeGuard: number | undefined

async function onWinFocus(focused: boolean) {
  if (!focused && !dragging) {
    if (askOpen.value) await closeAsk()
    if (panelOpen.value) await closePanel()
  }
}

async function guardSize() {
  if (panelOpen.value || askOpen.value || dragging) return
  try {
    const outer = await win.outerSize()
    // outerSize 是物理像素，窗口尺寸是逻辑像素，用 webview 的缩放比换算
    const scale = window.devicePixelRatio || 1
    const w = Math.round(outer.width / scale)
    const h = Math.round(outer.height / scale)
    if (w !== winW.value || h !== winH.value) {
      await win.setSize(new LogicalSize(winW.value, winH.value))
    }
  } catch {
    /* 忽略 */
  }
}

const finalOpacity = computed(() => (clickThrough.value ? Math.min(opacity.value, 0.4) : opacity.value))

// ==================== 菜单动作 ====================
async function menuShowMain() {
  await closePanel()
  await showMain()
}
async function menuQuickAsk() {
  await closePanel()
  void quickAsk()
}
async function quickAsk() {
  await openAsk()
}

const toolsList = ref<{ id: string; label: string; steps: number }[]>([])
async function goTools() {
  panelPage.value = 'tools'
  try {
    const res = await listToolboxItems()
    toolsList.value = res.items
      .filter((i) => i.enabled !== false)
      .map((i) => ({ id: i.id, label: i.name || i.id, steps: i.steps?.length || 0 }))
  } catch {
    toolsList.value = []
  }
}
async function runTool(id: string) {
  try {
    await executeToolbox(id)
    showNotice('已启动')
  } catch {
    showNotice('工具执行失败')
  }
}

async function switchMode(m: 'avatar' | 'simple' | 'live2d') {
  if (mode.value === m) return
  try {
    await setting.update({ floating_ball_mode: m })
  } catch {
    /* 忽略 */
  }
}

async function menuToggleMonitor() {
  const next = !monitorOn.value
  monitorOn.value = next
  try {
    await toggleMonitoring(next)
  } catch {
    /* 忽略 */
  }
}

async function menuToggleClickThrough() {
  const next = !clickThrough.value
  try {
    clickThrough.value = await setFloatingBallClickThrough(next)
  } catch {
    /* 忽略 */
  }
  await closePanel()
  if (clickThrough.value) showNotice('鼠标穿透已开启，点系统托盘图标可关闭')
}

async function menuExit() {
  await quitApp()
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault()
  if (!panelOpen.value) void openPanel()
}

// ==================== 生命周期 ====================
onMounted(async () => {
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)

  // 高 DPI 修复：这台机器 150% 缩放时，Tauri 无边框透明窗的 WebView2 会把物理像素
  // 当成 CSS 像素（内容整体放大约 1.5 倍、右下偏移）。按 DPR 反向缩放让画面回到窗口内。
  try {
    const dpr = window.devicePixelRatio || 1
    if (dpr > 1.01) {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
      await getCurrentWebviewWindow().setZoom(1 / dpr)
    }
  } catch {
    /* 忽略：缩放失败不影响桌宠功能 */
  }

  try {
    if (!setting.loaded) await setting.loadConfig()
  } catch {
    /* 忽略 */
  }

  unlistenCfg = await onConfigUpdated(async () => {
    try {
      await setting.loadConfig()
    } catch {
      /* 忽略 */
    }
  })
  unlistenCt = await onBallClickThroughChanged((st) => {
    clickThrough.value = st
  })
  unlistenFocus = await win.onFocusChanged(({ payload: focused }) => void onWinFocus(focused))
  sizeGuard = window.setInterval(() => void guardSize(), 5000)

  unlistenToggle = await listen('floating-ball-toggled', async () => {
    try {
      const vis = await win.isVisible()
      if (!vis && (panelOpen.value || askOpen.value)) {
        await restoreBall()
        panelOpen.value = false
        askOpen.value = false
      }
    } catch {
      /* 忽略 */
    }
  })

  unlistenAskHotkey = await listen('ball-hotkey-ask', () => {
    if (clickThrough.value) return
    void openAsk()
  })

  unlistenTrigger = await onMonitorTrigger((payload) => {
    // 屏幕监测提醒：直接冒气泡说一句，比闪烁更有桌宠的感觉
    hasMessage.value = true
    window.setTimeout(() => (hasMessage.value = false), 3000)
    const text = payload?.reply?.trim()
    say(text ? `（${payload.app_name || '屏幕'}）${text}` : '需要我帮忙吗？')
  })

  try {
    clickThrough.value = await getFloatingBallClickThrough()
  } catch {
    /* 忽略 */
  }
  try {
    const res = await getMonitorRules()
    monitorOn.value = res.enabled
  } catch {
    /* 忽略 */
  }

  try {
    // 位置不归前端管：窗口创建时 Rust 已按配置（或右下角默认）定位好。
    // 早期在这里 setPosition 会踩到「外壳与画面错位」，详见 lib.rs 的注释。
    await win.setSize(new LogicalSize(winW.value, winH.value))
    if (mode.value === 'live2d') {
      await nextTick()
      await loadLive2D()
    }
  } catch {
    /* 忽略 */
  }

  lastActive = Date.now()
  scheduleIdleAction()
  scheduleChatter()
  watchdog = window.setInterval(maybeSleep, 20_000)
})

onUnmounted(() => {
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
  if (longTimer) clearTimeout(longTimer)
  if (clickTimer) clearTimeout(clickTimer)
  if (bubbleTimer) clearTimeout(bubbleTimer)
  if (noticeTimer) clearTimeout(noticeTimer)
  if (actionTimer) clearTimeout(actionTimer)
  if (idleTimer) clearTimeout(idleTimer)
  if (chatterTimer) clearTimeout(chatterTimer)
  if (watchdog) clearInterval(watchdog)
  if (sizeGuard) clearInterval(sizeGuard)
  fireToMainTimers.forEach(clearTimeout)
  fireToMainTimers = []
  unlistenTrigger?.()
  unlistenCfg?.()
  unlistenCt?.()
  unlistenFocus?.()
  unlistenToggle?.()
  unlistenAskHotkey?.()
  askUnlisten.forEach((fn) => {
    try {
      fn()
    } catch {
      /* 忽略 */
    }
  })
  askUnlisten = []
  if (live2dCleanup) live2dCleanup()
})

// ==================== 模式/尺寸变化 ====================
watch(mode, async (m) => {
  if (panelOpen.value || askOpen.value) return
  try {
    if (m !== 'live2d' && live2dCleanup) {
      live2dCleanup()
      live2dCleanup = null
    }
    await win.setSize(new LogicalSize(winW.value, winH.value))
    if (m === 'live2d') {
      await nextTick()
      await loadLive2D()
    }
  } catch {
    /* 忽略 */
  }
})

watch(size, async () => {
  if (mode.value === 'live2d' || panelOpen.value) return
  try {
    await win.setSize(new LogicalSize(winW.value, winH.value))
  } catch {
    /* 忽略 */
  }
})

watch(enabled, async (val) => {
  try {
    // 位置已在创建时定好，显示/隐藏只切可见性
    if (val) await win.show()
    else await win.hide()
  } catch {
    /* 忽略 */
  }
})

// ==================== Live2D ====================
const live2dMount = ref<HTMLDivElement | null>(null)
const live2dReady = ref(false)
let live2dCleanup: (() => void) | null = null
let live2dRetries = 0
let live2dReloaded = false

async function loadLive2D() {
  if (mode.value !== 'live2d' || panelOpen.value) return
  live2dReady.value = false
  if (live2dCleanup) {
    live2dCleanup()
    live2dCleanup = null
  }
  await nextTick()
  // 模板还没挂上（首次渲染 / 热更新重挂载时可能撞上）→ 稍后重试，别就此空白
  if (!live2dMount.value) {
    if (live2dRetries < 5) {
      live2dRetries += 1
      window.setTimeout(() => {
        if (mode.value === 'live2d') void loadLive2D()
      }, 300)
    }
    return
  }
  live2dRetries = 0
  live2dMount.value.innerHTML = ''
  try {
    const { loadOml2d } = await import('oh-my-live2d')
    if (!live2dMount.value) return
    const container = live2dMount.value
    // 模型走前端静态资源（public/live2d，打包时进 dist）：
    // 之前用资源目录 + asset 协议，但资源在 F: 盘、assetProtocol scope 只放行 $HOME，
    // 模型实际加载不到 → 窗口一片透明。改由 webview 自身源加载，dev/打包都可靠。
    const modelUrl = `${import.meta.env.BASE_URL}live2d/haru/haru_greeter_t03.model3.json`

    let loaded = false
    let guard: number | undefined

    let oml2d: { onLoad: (fn: (s: string) => void) => void; destroy?: () => void } | null = null
    try {
      oml2d = loadOml2d({
        parentElement: container,
        statusBar: { disable: true },
        menus: { disable: true },
        models: [{ path: modelUrl, scale: 0.12 }],
      })
    } catch {
      showNotice('Live2D 不可用，先显示头像')
      return
    }
    oml2d?.onLoad((status) => {
      if (status === 'fail') {
        live2dReady.value = false
        showNotice('Live2D 加载失败，先显示头像')
        try {
          oml2d?.destroy?.()
        } catch {
          /* 忽略 */
        }
        return
      }
      live2dReady.value = true
    })
    // 兜底：8 秒还没起来就重建一次（WebGL 上下文偶尔首次初始化失败），
    // 再不行就保留底垫头像 —— 桌宠窗口绝不能是"隐形"的。
    guard = window.setTimeout(() => {
      const canvas = live2dMount.value?.querySelector('canvas') as HTMLCanvasElement | null
      if (loaded || (canvas && canvas.width > 0 && canvas.height > 0)) {
        live2dReady.value = true
        return
      }
      if (!live2dReloaded) {
        live2dReloaded = true
        void loadLive2D()
        return
      }
      showNotice('Live2D 没起来，先显示头像')
    }, 8000)
    live2dCleanup = () => {
      if (guard) clearTimeout(guard)
      try {
        oml2d?.destroy?.()
      } catch {
        /* 忽略 */
      }
      if (live2dMount.value) live2dMount.value.innerHTML = ''
    }
  } catch {
    if (live2dMount.value) live2dMount.value.innerHTML = '<div class="l2d-fail">Live2D 加载失败</div>'
  }
}
</script>

<template>
  <div class="pet-shell" @mousedown="onMouseDown" @contextmenu="onContextMenu">
    <!-- ====== 收起态：台词气泡 + 角色 ====== -->
    <template v-if="!panelOpen && !askOpen">
      <Transition name="speech">
        <div v-if="bubbleOn" class="speech" :style="{ opacity: finalOpacity }">{{ bubble }}</div>
      </Transition>

      <div class="pet-stage" :class="stageClass" :style="{ opacity: finalOpacity }">
        <div v-if="mode === 'live2d'" class="live2d-container">
          <!-- 底垫头像：Live2D 还没起来时至少有形象，不让桌面出现一个"隐形窗口" -->
          <img v-show="!live2dReady" :src="ballAvatar" class="pet-avatar l2d-backdrop" draggable="false" alt="铃" />
          <div ref="live2dMount" class="live2d-mount"></div>
        </div>
        <img v-else-if="mode === 'avatar'" :src="ballAvatar" class="pet-avatar" draggable="false" alt="铃" />
        <div v-else class="pet-text">{{ setting.selfName || '铃' }}</div>
      </div>

      <div v-if="zzz" class="zzz">z<span>z</span><span>z</span></div>
      <div v-if="hasMessage" class="msg-badge"></div>
      <div v-if="clickThrough" class="ct-badge">穿透中 · 点托盘关闭</div>
      <Transition name="fade">
        <div v-if="notice" class="notice-tip">{{ notice }}</div>
      </Transition>
    </template>

    <!-- ====== 展开态一：右键控制台 ====== -->
    <div v-else-if="panelOpen" class="panel">
      <div class="panel-head">
        <span class="mini-ball">
          <img v-if="mode === 'avatar'" :src="ballAvatar" class="avatar-img" draggable="false" />
          <span v-else class="mini-text">{{ setting.selfName || '铃' }}</span>
        </span>
        <span class="panel-title">{{ panelPage === 'tools' ? '小工具' : setting.selfName || '铃' }}</span>
        <span class="panel-dot" :class="monitorOn ? 'on' : 'off'" :title="monitorOn ? '屏幕监测中' : '监测已暂停'"></span>
      </div>

      <div class="panel-body">
        <template v-if="panelPage === 'main'">
          <div class="p-item" @click="menuShowMain">打开主窗口</div>
          <div class="p-item" @click="menuQuickAsk">快速提问</div>
          <div class="p-item" @click="goTools">工具箱<span class="p-hint">小工具 ›</span></div>

          <div class="p-sep"></div>

          <div class="p-seg-title">形象</div>
          <div class="p-seg">
            <div class="seg-btn" :class="{ active: mode === 'live2d' }" @click="switchMode('live2d')">Live2D</div>
            <div class="seg-btn" :class="{ active: mode === 'avatar' }" @click="switchMode('avatar')">头像</div>
            <div class="seg-btn" :class="{ active: mode === 'simple' }" @click="switchMode('simple')">文字</div>
          </div>

          <div class="p-sep"></div>

          <div class="p-item" @click="menuToggleClickThrough">
            {{ clickThrough ? '关闭鼠标穿透' : '鼠标穿透' }}
            <span class="p-hint">{{ clickThrough ? '已开启' : '' }}</span>
          </div>
          <div class="p-item" @click="menuToggleMonitor">
            {{ monitorOn ? '暂停屏幕监测' : '恢复屏幕监测' }}
            <span class="p-hint">{{ monitorOn ? '监测中' : '已暂停' }}</span>
          </div>

          <div class="p-sep"></div>

          <div class="p-item" @click="openSettings">设置…</div>
          <div class="p-item danger" @click="menuExit">退出</div>
        </template>

        <template v-else>
          <div class="p-item" @click="panelPage = 'main'"><span class="back-arrow">‹</span> 返回</div>
          <div class="p-sep"></div>
          <div class="tools-grid">
            <div v-for="t in toolsList" :key="t.id" class="tool-btn" @click="runTool(t.id)">
              {{ t.label }}<span v-if="t.steps > 0" class="tool-badge">×{{ t.steps }}</span>
            </div>
            <div v-if="toolsList.length === 0" class="tools-hint">工具箱暂无可用工具，可在主窗口设置中添加</div>
          </div>
          <div class="tools-hint">快捷工具启动后自动收起</div>
        </template>
      </div>
    </div>

    <!-- ====== 展开态二：快速提问迷你对话卡 ====== -->
    <div v-else-if="askOpen" class="ask-card">
      <div class="ask-head">
        <span class="ask-title">{{ setting.selfName || '铃' }} · 快速提问</span>
        <span class="ask-close" @click="closeAsk">×</span>
      </div>
      <div ref="askBodyRef" class="ask-body">
        <div v-if="askMsgs.length === 0 && !askStreaming" class="ask-empty">在这里直接和铃对话，回复实时显示～</div>
        <div v-for="(m, i) in askMsgs" :key="i" class="msg-line" :class="m.role === 'user' ? 'from-user' : 'from-suzu'">{{ m.text }}</div>
        <div v-if="askStreaming" class="msg-line from-suzu streaming">{{ askStreaming }}<span class="caret"></span></div>
        <div v-if="askBusy && !askStreaming" class="msg-line from-suzu typing">铃正在思考…</div>
      </div>
      <div class="ask-row">
        <input
          ref="askInput"
          v-model="askText"
          class="ask-input"
          placeholder="问点什么… Enter 发送"
          :disabled="askBusy"
          @compositionstart="askComposing = true"
          @compositionend="askComposing = false"
          @keydown.enter.prevent="onAskEnter"
        />
        <button class="ask-send" :disabled="askBusy || !askText.trim()" @click="submitAsk">发送</button>
      </div>
      <div class="ask-sub">对话会同步保存到最近会话</div>
    </div>
  </div>
</template>

<style scoped>
.pet-shell {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  position: relative;
  user-select: none;
  -webkit-user-select: none;
  font-family: var(--font-main, system-ui, 'Microsoft YaHei', sans-serif);
}

/* ===== 台词气泡（角色头顶，带小尾巴） ===== */
.speech {
  position: absolute;
  top: 1.5%;
  left: 3%;
  right: 3%;
  padding: 2.5% 3%;
  border-radius: 12px;
  background: color-mix(in srgb, var(--bg-bar, rgba(34, 32, 36, 0.92)) 92%, transparent);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  color: var(--text-main, #eee6e7);
  font-size: clamp(10px, 4.4vmin, 16px);
  line-height: 1.45;
  text-align: center;
  word-break: break-word;
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  pointer-events: none;
  z-index: 4;
}
.speech::after {
  content: '';
  position: absolute;
  bottom: -6px;
  left: 50%;
  margin-left: -6px;
  border: 6px solid transparent;
  border-top-color: color-mix(in srgb, var(--bg-bar, rgba(34, 32, 36, 0.92)) 92%, transparent);
  border-bottom: 0;
}
.speech-enter-active,
.speech-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.speech-enter-from,
.speech-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

/* ===== 角色区（用视口百分比布局：高 DPI 下 webview 视口按物理像素算，
        写死 px 会跟窗口对不上，百分比/视口单位才能自适应） ===== */
.pet-stage {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 76%;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  cursor: grab;
  transform-origin: 50% 92%;
  transition: opacity 0.25s;
}
.pet-stage:active {
  cursor: grabbing;
}
.pet-stage.dim {
  filter: saturate(0.75) brightness(0.88);
}
.pet-avatar {
  width: 88%;
  height: 88%;
  object-fit: cover;
  border-radius: 50%;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.22);
  pointer-events: none;
}
.pet-text {
  width: 82%;
  height: 82%;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24vmin;
  font-weight: 600;
  color: #fff;
  background: linear-gradient(135deg, var(--accent, #ff7a94), color-mix(in srgb, var(--accent, #ff7a94) 55%, #6db3ff));
  border: 2px solid rgba(255, 255, 255, 0.7);
  pointer-events: none;
}
.live2d-container {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}
/* oml2d 在高 DPI（本机 150%）下会把 canvas 按物理像素撑开，溢出本就不大的桌宠窗口。
   强制按容器尺寸显示，角色才会完整待在窗口里。 */
.live2d-mount :deep(canvas) {
  width: 100% !important;
  height: 100% !important;
  display: block;
}
.live2d-container .l2d-backdrop {
  position: absolute;
  inset: 0;
  margin: auto;
  width: 72%;
  height: 72%;
  border-radius: 50%;
}
.live2d-mount {
  width: 100%;
  height: 100%;
}
.l2d-fail {
  color: var(--text-secondary, #999);
  font-size: var(--fs-12, 12px);
  text-align: center;
  padding: 20px;
}

/* 待机呼吸（轻微缩放，不动画宽度） */
.breathing {
  animation: pet-breathe 3.2s ease-in-out infinite;
}
@keyframes pet-breathe {
  0%, 100% { transform: scale(1); }
  50% { transform: scale(1.018); }
}

/* 被戳：弹一下 + 左右晃 */
.state-poke {
  animation: pet-poke 0.42s ease-out;
}
@keyframes pet-poke {
  0% { transform: scale(1); }
  30% { transform: scale(0.94) rotate(-3deg); }
  60% { transform: scale(1.05) rotate(2deg); }
  100% { transform: scale(1); }
}

/* 被摸头：舒服地上下浮 */
.state-pat {
  animation: pet-pat 0.9s ease-in-out;
}
@keyframes pet-pat {
  0%, 100% { transform: translateY(0) rotate(0); }
  25% { transform: translateY(-4px) rotate(-1.5deg); }
  55% { transform: translateY(2px) rotate(1deg); }
  80% { transform: translateY(-2px) rotate(-0.5deg); }
}

/* 被拎起来：歪着轻轻晃 */
.state-drag {
  animation: pet-drag 1.1s ease-in-out infinite;
  cursor: grabbing;
}
@keyframes pet-drag {
  0%, 100% { transform: rotate(-4deg); }
  50% { transform: rotate(4deg); }
}

/* 下落：先仰后正 */
.state-fall {
  animation: pet-fall 0.5s ease-out infinite alternate;
}
@keyframes pet-fall {
  from { transform: rotate(-2deg) scale(0.99); }
  to { transform: rotate(1deg) scale(1.01); }
}

/* 待机小动作：小小跳一下 */
.state-hop {
  animation: pet-hop 0.6s ease-out;
}
@keyframes pet-hop {
  0% { transform: translateY(0); }
  35% { transform: translateY(-7px) scale(1.01); }
  70% { transform: translateY(1px); }
  100% { transform: translateY(0); }
}

/* 打瞌睡 */
.state-sleep {
  animation: pet-sleep 4s ease-in-out infinite;
}
@keyframes pet-sleep {
  0%, 100% { transform: rotate(-2deg) scale(0.99); }
  50% { transform: rotate(-2deg) scale(1.005); }
}

/* zzz 小字 */
.zzz {
  position: absolute;
  top: 22%;
  right: 16%;
  font-size: var(--fs-14, 14px);
  font-weight: 600;
  color: var(--text-main, #eee6e7);
  opacity: 0.85;
  z-index: 5;
  pointer-events: none;
}
.zzz span {
  margin-left: 2px;
  animation: zzz-rise 2.2s ease-out infinite;
  display: inline-block;
}
.zzz span:nth-child(2) { animation-delay: 0.5s; opacity: 0.7; }
.zzz span:nth-child(3) { animation-delay: 1s; opacity: 0.45; }
@keyframes zzz-rise {
  0% { transform: translateY(0); opacity: 0.9; }
  100% { transform: translateY(-14px); opacity: 0.1; }
}

/* 消息角标 */
.msg-badge {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--danger, #ff6b6b);
  border: 2px solid rgba(255, 255, 255, 0.85);
  animation: badge-pulse 0.8s ease-in-out infinite;
  z-index: 5;
}
@keyframes badge-pulse {
  0%, 100% { transform: scale(1); opacity: 1; }
  50% { transform: scale(1.35); opacity: 0.75; }
}

.ct-badge {
  position: absolute;
  top: 6px;
  left: 50%;
  transform: translateX(-50%);
  padding: 1px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--bg-bar, #222024) 82%, transparent);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  color: var(--text-main, #eee6e7);
  font-size: var(--fs-10, 10px);
  line-height: 16px;
  white-space: nowrap;
  z-index: 6;
  pointer-events: none;
  backdrop-filter: blur(6px);
}

.notice-tip {
  position: absolute;
  bottom: 8px;
  left: 50%;
  transform: translateX(-50%);
  max-width: calc(100% - 16px);
  padding: 3px 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--bg-bar, #222024) 88%, transparent);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  color: var(--text-main, #eee6e7);
  font-size: var(--fs-11, 11px);
  line-height: 18px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  z-index: 7;
  pointer-events: none;
  backdrop-filter: blur(6px);
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* ===== 控制台 ===== */
.panel {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-bar, rgba(34, 32, 36, 0.92));
  border-radius: var(--radius-ui, 14px);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  overflow: hidden;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}
.panel-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  flex-shrink: 0;
}
.mini-ball {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, var(--accent, #ff7a94), color-mix(in srgb, var(--accent, #ff7a94) 55%, #6db3ff));
  border: 1.5px solid rgba(255, 255, 255, 0.6);
}
.mini-text {
  font-size: var(--fs-13, 13px);
  font-weight: 600;
  color: #fff;
}
.panel-title {
  font-size: var(--fs-13, 13px);
  font-weight: 600;
  color: var(--text-main, #eee6e7);
  flex: 1;
}
.panel-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.panel-dot.on {
  background: var(--success, #7fd99a);
  box-shadow: 0 0 6px var(--success, #7fd99a);
}
.panel-dot.off {
  background: var(--text-secondary, #9a9294);
}
.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
}
.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
}
.p-item {
  padding: 8px 10px;
  border-radius: calc(var(--radius-ui, 14px) - 4px);
  font-size: var(--fs-13, 13px);
  color: var(--text-main, #eee6e7);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}
.p-item:hover {
  background: color-mix(in srgb, var(--accent, #ff7a94) 18%, transparent);
}
.p-item.danger {
  color: var(--danger, #ff6b6b);
}
.p-hint {
  margin-left: auto;
  font-size: var(--fs-11, 11px);
  color: var(--text-secondary, #9a9294);
}
.p-sep {
  height: 1px;
  margin: 5px 8px;
  background: var(--border, rgba(255, 255, 255, 0.08));
}
.p-seg-title {
  padding: 4px 10px 2px;
  font-size: var(--fs-11, 11px);
  color: var(--text-secondary, #9a9294);
}
.p-seg {
  display: flex;
  gap: 4px;
  padding: 2px 8px 6px;
}
.seg-btn {
  flex: 1;
  text-align: center;
  padding: 5px 0;
  border-radius: calc(var(--radius-ui, 14px) - 6px);
  font-size: var(--fs-12, 12px);
  color: var(--text-secondary, #9a9294);
  background: color-mix(in srgb, var(--input-bg, #2a272b) 80%, transparent);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  cursor: pointer;
  transition: all 0.15s;
}
.seg-btn:hover {
  color: var(--text-main, #eee6e7);
}
.seg-btn.active {
  color: #fff;
  background: var(--accent, #ff7a94);
  border-color: var(--accent, #ff7a94);
  font-weight: 600;
}
.back-arrow {
  font-size: var(--fs-16, 16px);
  line-height: 1;
  margin-right: 2px;
}
.tools-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
  padding: 6px 8px;
}
.tool-btn {
  padding: 10px 6px;
  text-align: center;
  border-radius: calc(var(--radius-ui, 14px) - 6px);
  font-size: var(--fs-12, 12px);
  color: var(--text-main, #eee6e7);
  background: color-mix(in srgb, var(--input-bg, #2a272b) 80%, transparent);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.tool-btn:hover {
  color: #fff;
  background: var(--accent, #ff7a94);
  border-color: var(--accent, #ff7a94);
}
.tool-badge {
  margin-left: 4px;
  font-size: var(--fs-10, 10px);
  opacity: 0.75;
}
.tools-hint {
  padding: 2px 10px;
  font-size: var(--fs-11, 11px);
  color: var(--text-secondary, #9a9294);
  text-align: center;
}

/* ===== 快速提问卡 ===== */
.ask-card {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-bar, rgba(34, 32, 36, 0.94));
  border-radius: var(--radius-ui, 14px);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  overflow: hidden;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  box-sizing: border-box;
}
.ask-head {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  flex-shrink: 0;
}
.ask-title {
  font-size: var(--fs-13, 13px);
  font-weight: 600;
  color: var(--text-main, #eee6e7);
  flex: 1;
}
.ask-close {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-size: var(--fs-14, 14px);
  line-height: 1;
  color: var(--text-secondary, #9a9294);
  cursor: pointer;
}
.ask-close:hover {
  background: color-mix(in srgb, var(--danger, #ff6b6b) 25%, transparent);
  color: var(--danger, #ff6b6b);
}
.ask-row {
  display: flex;
  gap: 6px;
  padding: 10px 12px;
  flex-shrink: 0;
}
.ask-input {
  flex: 1;
  min-width: 0;
  padding: 8px 10px;
  border-radius: calc(var(--radius-ui, 14px) - 6px);
  border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
  background: var(--input-bg, #2a272b);
  color: var(--text-main, #eee6e7);
  font-size: var(--fs-13, 13px);
  font-family: inherit;
  outline: none;
}
.ask-input:focus {
  border-color: var(--accent, #ff7a94);
}
.ask-send {
  padding: 0 14px;
  border: none;
  border-radius: calc(var(--radius-ui, 14px) - 6px);
  background: var(--accent, #ff7a94);
  color: #fff;
  font-size: var(--fs-13, 13px);
  cursor: pointer;
  flex-shrink: 0;
}
.ask-send:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.ask-sub {
  padding: 0 12px 8px;
  font-size: var(--fs-11, 11px);
  color: var(--text-secondary, #9a9294);
  flex-shrink: 0;
}
.ask-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.ask-empty {
  font-size: var(--fs-12, 12px);
  color: var(--text-secondary, #9a9294);
  text-align: center;
  padding: 14px 0;
}
.msg-line {
  max-width: 92%;
  padding: 6px 10px;
  border-radius: calc(var(--radius-ui, 14px) - 4px);
  font-size: var(--fs-12, 12px);
  line-height: 1.55;
  word-break: break-word;
  white-space: pre-wrap;
}
.msg-line.from-user {
  align-self: flex-end;
  background: var(--bubble-user-bg, #3a3438);
  color: var(--text-user, #fff);
  border-bottom-right-radius: 4px;
}
.msg-line.from-suzu {
  align-self: flex-start;
  background: color-mix(in srgb, var(--input-bg, #2a272b) 75%, transparent);
  color: var(--text-main, #eee6e7);
  border-bottom-left-radius: 4px;
}
.msg-line.streaming .caret {
  display: inline-block;
  width: 2px;
  height: 1em;
  margin-left: 2px;
  vertical-align: text-bottom;
  background: var(--accent, #ff7a94);
  animation: caret-blink 0.8s step-end infinite;
}
@keyframes caret-blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}
.msg-line.typing {
  color: var(--text-secondary, #9a9294);
  font-style: italic;
}
</style>

<style>
html,
body,
#app,
.app-root {
  background: transparent !important;
  overflow: hidden !important;
  width: 100% !important;
  height: 100% !important;
  margin: 0 !important;
  padding: 0 !important;
}
html::-webkit-scrollbar,
body::-webkit-scrollbar,
#app::-webkit-scrollbar,
*::-webkit-scrollbar {
  display: none !important;
  width: 0 !important;
  height: 0 !important;
}
</style>
