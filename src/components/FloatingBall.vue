<!-- 《铃·记忆体》悬浮球 v7（v0.6 重构）
     交互：10px 拖动阈值 + 边缘磁吸动画 + 单击唤起主窗 / 双击快速提问 / 长按蹭蹭动画
     右键：灵动展开面板（模式切换/穿透/监测/设置，全主题化样式）
     反馈：消息角标闪烁 + 穿透状态胶囊 + 配置实时同步 + 鼠标穿透（托盘可恢复）
     修复：悬浮球设置项此前永不生效（独立窗口未加载配置）→ 挂载即 loadConfig + 监听 config-updated -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { resourceDir } from '@tauri-apps/api/path'
import { currentMonitor, getCurrentWindow, LogicalPosition, LogicalSize, primaryMonitor } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { emit, listen } from '@tauri-apps/api/event'
import {
  ensureMainWindow,
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
  sendMessage,
  setFloatingBallClickThrough,
  toggleMonitoring,
} from '../utils/tauri'
import { useSettingStore } from '../stores/settingStore'
import ballAvatar from '../assets/ball_avatar.png'

const win = getCurrentWindow()
const setting = useSettingStore()

const mode = computed(() => setting.floatingBallMode)
const enabled = computed(() => setting.floatingBallEnabled)
const size = computed(() => setting.floatingBallSize)
const opacity = computed(() => setting.floatingBallOpacity)
const breathing = computed(() => setting.floatingBallBreathing)
const flash = computed(() => setting.floatingBallFlash)

const hasMessage = ref(false)
const clickThrough = ref(false)
const monitorOn = ref(true)
let flashTimer: number | undefined
let unlistenTrigger: (() => void) | undefined
let unlistenCfg: (() => void) | undefined
let unlistenCt: (() => void) | undefined
let unlistenFocus: (() => void) | undefined
let unlistenToggle: (() => void) | undefined
let unlistenAskHotkey: (() => void) | undefined
let sizeGuard: number | undefined

// ==================== 常量 ====================
const DRAG_THRESHOLD = 10 // 拖动判定阈值（px，防误触）
const CLICK_WINDOW = 280 // 双击判定间隔（ms）
const LONG_PRESS_MS = 600 // 长按触发时长
const SNAP_DIST = 26 // 边缘磁吸距离（px）
const SNAP_FRAMES = 14 // 磁吸动画帧数
const PANEL_W = 232 // 展开面板宽
const PANEL_H = 344 // 展开面板高
const ASK_W = 300 // 快速提问卡宽
const ASK_H = 320 // 快速提问卡高（含对话区）

// ==================== 拖拽（v7.1：系统级拖动 startDragging，OS 接管，跟手/防丢） ====================
let dragging = false
let moved = false
let osDragging = false // 系统拖动进行中（OS 接管，松手由系统回调）
let pressX = 0
let pressY = 0
let snapRaf: number | null = null
let longTimer: number | undefined
let cuddleTimer: number | undefined
let clickTimer: number | undefined
let lastClickAt = 0

// —— 位置持久化（v8：v7 曾误存物理像素，@150% 缩放下重启即漂移；v8 起统一存逻辑坐标）——
const POS_KEY = 'floating-ball-pos-v8'

/** 屏幕缩放（物理/逻辑 比值） */
async function dpiScale(): Promise<number> {
  try {
    const mon = (await currentMonitor()) || (await primaryMonitor())
    return mon?.scaleFactor || 1
  } catch { return 1 }
}

/** 当前窗口位置（逻辑坐标）——outerPosition 返回物理像素，必须 ÷scale 统一为逻辑 */
async function winLogicalPos(): Promise<{ x: number; y: number }> {
  const p = await win.outerPosition()
  const s = await dpiScale()
  return { x: Math.round(p.x / s), y: Math.round(p.y / s) }
}

/** 当前模式的窗口尺寸 */
function currentWinSize(): number {
  return mode.value === 'live2d' ? 300 : size.value
}

/** 逻辑坐标约束在屏幕内（w/h 为逻辑宽高） */
async function clampRect(x: number, y: number, w: number, h: number): Promise<[number, number]> {
  try {
    const monitor = (await currentMonitor()) || (await primaryMonitor())
    if (monitor) {
      const scale = monitor.scaleFactor || 1
      const lw = Math.round(monitor.size.width / scale)
      const lh = Math.round(monitor.size.height / scale)
      return [
        Math.min(Math.max(0, x), Math.max(0, lw - w)),
        Math.min(Math.max(0, y), Math.max(0, lh - h)),
      ]
    }
  } catch { /* 忽略 */ }
  return [x, y]
}

async function clampToScreen(x: number, y: number): Promise<[number, number]> {
  const s = currentWinSize()
  return await clampRect(x, y, s, mode.value === 'live2d' ? 400 : s)
}

// ==================== 鼠标事件 ====================
function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  // 展开面板状态不启动拖拽
  if (panelOpen.value) return
  e.preventDefault()
  dragging = true
  moved = false
  osDragging = false
  pressX = e.screenX
  pressY = e.screenY
  // 上一次磁吸动画若未结束，立即取消（防止与本次拖动抢位置 → 瞬移）
  if (snapRaf !== null) { cancelAnimationFrame(snapRaf); snapRaf = null }
  // 长按计时：600ms 未拖动未弹起 → 蹭蹭动画
  if (longTimer) clearTimeout(longTimer)
  longTimer = window.setTimeout(() => {
    if (dragging && !moved && !osDragging && !panelOpen.value) {
      playCuddle()
    }
  }, LONG_PRESS_MS)
}

function onMouseMove(e: MouseEvent) {
  if (!dragging || osDragging || moved) return
  // 位移超过 10px 阈值 → 判定为拖动，交给系统级拖动（OS 接管，跟手且不出窗丢失）
  const dx = e.screenX - pressX
  const dy = e.screenY - pressY
  if (Math.hypot(dx, dy) < DRAG_THRESHOLD) return
  moved = true
  if (longTimer) { clearTimeout(longTimer); longTimer = undefined }
  startOsDrag()
}

/** 系统级拖动：阻塞至松手，结束后磁吸 + 保存位置 */
async function startOsDrag() {
  if (osDragging) return
  osDragging = true
  try {
    await win.startDragging()
  } catch { /* 忽略（如非左键拖动被系统拒绝） */ }
  osDragging = false
  dragging = false
  if (longTimer) { clearTimeout(longTimer); longTimer = undefined }
  // 松手：边缘磁吸 + 持久化位置（逻辑坐标）
  winLogicalPos().then((p) => snapAndSave(p.x, p.y)).catch(() => {})
}

function onMouseUp() {
  // 系统拖动期间松手由 OS 处理，这里不重复判定
  if (osDragging) return
  if (!dragging) return
  dragging = false
  if (longTimer) { clearTimeout(longTimer); longTimer = undefined }
  // 纯点击（未达到拖动阈值，窗口从未被移动）：仅做单击/双击判定
  if (!moved) {
    handleClick()
  }
}

/** 单击/双击判定：双击(280ms 内二次点击)→快速提问；单击→轻交互（主窗可见则聚焦） */
function handleClick() {
  const now = Date.now()
  if (now - lastClickAt < CLICK_WINDOW) {
    // 双击
    if (clickTimer) { clearTimeout(clickTimer); clickTimer = undefined }
    lastClickAt = 0
    quickAsk()
    return
  }
  lastClickAt = now
  if (clickTimer) clearTimeout(clickTimer)
  clickTimer = window.setTimeout(() => {
    clickTimer = undefined
    lastClickAt = 0
    focusMain()
    playPop()
  }, 250)
}

// ==================== 边缘磁吸动画（ease-out 平滑滑向边缘） ====================
async function snapAndSave(x: number, y: number) {
  const s = currentWinSize()
  const h = mode.value === 'live2d' ? 400 : s
  const [cx, cy] = await clampRect(x, y, s, h)
  let tx = cx
  let ty = cy
  try {
    const monitor = (await currentMonitor()) || (await primaryMonitor())
    if (monitor) {
      const scale = monitor.scaleFactor || 1
      const lw = Math.round(monitor.size.width / scale)
      const lh = Math.round(monitor.size.height / scale)
      // 距哪条边近就吸哪条（磁吸距离 SNAP_DIST）
      const edges = [
        { d: x, set: () => (tx = 0) },
        { d: lw - s - x, set: () => (tx = lw - s) },
        { d: y, set: () => (ty = 0) },
        { d: lh - h - y, set: () => (ty = lh - h) },
      ]
      edges.sort((a, b) => a.d - b.d)
      if (edges[0].d < SNAP_DIST) edges[0].set()
    }
  } catch { /* 忽略 */ }
  await animateTo(tx, ty, SNAP_FRAMES)
  // 保存逻辑坐标
  winLogicalPos().then((p) => {
    try {
      localStorage.setItem(POS_KEY, JSON.stringify({ x: p.x, y: p.y }))
    } catch { /* 忽略 */ }
  }).catch(() => {})
}

/** 逐帧平移动画（ease-out；起点取逻辑坐标） */
function animateTo(x: number, y: number, frames: number): Promise<void> {
  return new Promise((resolve) => {
    if (snapRaf !== null) { cancelAnimationFrame(snapRaf); snapRaf = null }
    winLogicalPos().then((p0) => {
      const sx = p0.x
      const sy = p0.y
      const dx = x - sx
      const dy = y - sy
      let step = 1
      const tick = () => {
        const t = step / frames
        const ease = 1 - Math.pow(1 - t, 3)
        win.setPosition(new LogicalPosition(Math.round(sx + dx * ease), Math.round(sy + dy * ease))).catch(() => {})
        if (step >= frames) { snapRaf = null; resolve(); return }
        step += 1
        snapRaf = window.requestAnimationFrame(tick)
      }
      tick()
    }).catch(() => resolve())
  })
}

/** 唤起主窗口（主窗被关则重建；最小化则还原；弹出后悬浮球重新置顶防被盖） */
async function showMain() {
  let main = await WebviewWindow.getByLabel('main').catch(() => null)
  if (!main) {
    try { await ensureMainWindow() } catch { /* 重建失败忽略 */ }
    main = await WebviewWindow.getByLabel('main').catch(() => null)
  }
  emit('ball-diag', `showMain main=${main ? 'exists' : 'MISSING'}`).catch(() => {})
  if (main) {
    await main.unminimize().catch(() => {})
    await main.show().catch(() => {})
    await main.setFocus().catch(() => {})
  }
  // 关键：透明小窗的置顶位在别的窗口激活后容易丢，重新置顶确保球始终在最上层
  try { await win.setAlwaysOnTop(true) } catch { /* 忽略 */ }
}

/** 调试：把当前窗口位置（逻辑）/尺寸（物理原始+逻辑换算）上报到 Rust 日志（v0.6 开发期用） */
async function diag(tag: string) {
  try {
    const p = await winLogicalPos()
    const sz = await win.outerSize()
    const s = await dpiScale()
    emit('ball-diag', `${tag} pos=${p.x},${p.y} sizePhys=${sz.width}x${sz.height} sizeLog=${Math.round(sz.width / s)}x${Math.round(sz.height / s)} scale=${s}`).catch(() => {})
  } catch { /* 忽略 */ }
}

/** 单击轻交互：主窗口已显示则聚焦；未显示则不弹窗（避免大窗盖住球造成"球消失"），只做引导提示 */
async function focusMain() {
  try {
    const main = await WebviewWindow.getByLabel('main')
    if (main) {
      const vis = await main.isVisible().catch(() => false)
      if (vis) {
        await main.setFocus()
        return
      }
      showHint()
    }
  } catch { /* 忽略 */ }
}

/** 引导提示（带冷却，防每次单击刷屏） */
let hintAt = 0
function showHint() {
  const now = Date.now()
  if (now - hintAt < 4000) return
  hintAt = now
  showNotice('双击快速提问 · 右键更多操作')
}

/** 向主窗口持续补发事件（主窗被关重建/冷启动时会错过早期事件，多档间隔兜底） */
function fireToMain(eventName: string) {
  const delays = [120, 450, 850, 1300, 1900]
  for (const d of delays) {
    setTimeout(() => { emit(eventName).catch(() => {}) }, d)
  }
}

/** 双击：快速提问（球边弹迷你输入卡，不唤起主窗口） */
function quickAsk() {
  openAsk()
}

/** 菜单/面板：打开主窗口设置页（事件多档补发防错过） */
async function openSettings() {
  await showMain()
  fireToMain('floating-open-settings')
}

// ==================== 长按蹭蹭动画 ====================
const cuddling = ref(false)
function playCuddle() {
  cuddling.value = true
  if (cuddleTimer) clearTimeout(cuddleTimer)
  cuddleTimer = window.setTimeout(() => (cuddling.value = false), 1200)
}

// ==================== 点击弹跳反馈 ====================
const popping = ref(false)
let popTimer: number | undefined
function playPop() {
  popping.value = true
  if (popTimer) clearTimeout(popTimer)
  popTimer = window.setTimeout(() => (popping.value = false), 320)
}

// ==================== 右键灵动面板（悬浮球展开为控制面板） ====================
const panelOpen = ref(false)
const askOpen = ref(false) // 快速提问迷你卡（与 panel 互斥）
const panelPage = ref<'main' | 'tools'>('main') // 面板内页：主菜单 / 小工具
const askText = ref('')
const askInput = ref<HTMLInputElement | null>(null)
// —— 迷你对话：在提问卡内直接展示铃的回复（不依赖主窗口）——
const askMsgs = ref<{ role: 'user' | 'suzu'; text: string }[]>([])
const askStreaming = ref('')
const askBusy = ref(false)
let askUnlisten: (() => void)[] = []
let panelPrevPos = { x: 0, y: 0 }

/** 展开态通用：以球为中心展开到 w×h（互斥关闭另一展开态；记录球位置用于恢复） */
async function expandTo(w: number, h: number, kind: 'panel' | 'ask') {
  // 互斥：先把另一展开态标记清掉（窗口形态随后统一 resize，不重复走各自恢复）
  if (kind === 'ask' && panelOpen.value) panelOpen.value = false
  if (kind === 'panel' && askOpen.value) askOpen.value = false
  if (kind === 'ask') askOpen.value = true
  if (kind === 'panel') panelOpen.value = true
  // live2d：先卸模型，避免盖住时残留渲染
  if (mode.value === 'live2d' && live2dCleanup) live2dCleanup()
  await diag(`expand-${kind}-before`)
  try {
    const pos = await winLogicalPos()
    panelPrevPos = { x: pos.x, y: pos.y }
    await diag(`expand-${kind}-pos(${pos.x},${pos.y})`)
    // 中心对齐展开：面板左上 = 球左上 + (球尺寸-目标尺寸)/2，再 clamp 到屏内
    const s = currentWinSize()
    const sh = mode.value === 'live2d' ? 400 : s
    const [cx, cy] = await clampRect(
      pos.x + Math.round((s - w) / 2),
      pos.y + Math.round((sh - h) / 2),
      w,
      h,
    )
    // 双保险：先归位 → 放大 → 再归位（吞掉 Windows setSize 缩放锚偏移）
    await win.setPosition(new LogicalPosition(cx, cy))
    await win.setSize(new LogicalSize(w, h))
    await win.setPosition(new LogicalPosition(cx, cy))
  } catch { /* 忽略 */ }
  await diag(`expand-${kind}-after`)
}

/** 收起展开态：恢复球尺寸 + 回到球原位置 */
async function restoreBall() {
  try {
    const s = currentWinSize()
    const [cx, cy] = await clampToScreen(panelPrevPos.x, panelPrevPos.y)
    // 双保险归位：先归位 → 收缩尺寸 → 再归位一次
    await win.setPosition(new LogicalPosition(cx, cy))
    await win.setSize(new LogicalSize(s, mode.value === 'live2d' ? 400 : s))
    await win.setPosition(new LogicalPosition(cx, cy))
  } catch { /* 忽略 */ }
  await diag('restoreBall-after')
  if (mode.value === 'live2d') {
    nextTick(() => loadLive2D())
  }
}

async function openPanel() {
  if (panelOpen.value) return
  panelPage.value = 'main'
  await expandTo(PANEL_W, PANEL_H, 'panel')
  // 刷新监测状态显示
  try {
    const res = await getMonitorRules()
    monitorOn.value = res.enabled
  } catch { /* 忽略 */ }
}

async function closePanel() {
  if (!panelOpen.value) return
  panelOpen.value = false
  await restoreBall()
}

// —— 快速提问迷你对话卡：双击 / 面板「快速提问」/ 全局热键 Ctrl+Alt+Q ——
async function openAsk() {
  if (askOpen.value) return
  askText.value = ''
  await expandTo(ASK_W, ASK_H, 'ask')
  // 注册回复流式监听（仅提问卡打开期间；回复经 chat_chunk/chat_end 全局广播）
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
  } catch { /* 忽略 */ }
  // 聚焦输入框（等窗口形态稳定后）
  setTimeout(() => askInput.value?.focus(), 180)
}

async function closeAsk() {
  if (!askOpen.value) return
  askOpen.value = false
  askBusy.value = false
  askStreaming.value = ''
  askUnlisten.forEach((fn) => { try { fn() } catch { /* 忽略 */ } })
  askUnlisten = []
  await restoreBall()
}

const askBodyRef = ref<HTMLDivElement | null>(null)
function scrollAskToBottom() {
  setTimeout(() => {
    if (askBodyRef.value) askBodyRef.value.scrollTop = askBodyRef.value.scrollHeight
  }, 30)
}

/** 发送快速提问：消息与回复都在提问卡内对话展示（同时存档最近会话） */
async function submitAsk() {
  const text = askText.value.trim()
  if (!text || askBusy.value) return
  askText.value = ''
  askMsgs.value.push({ role: 'user', text })
  scrollAskToBottom()
  askBusy.value = true
  askStreaming.value = ''
  let sid: string | null = null
  try {
    const sessions = await listSessions()
    if (sessions && sessions.length > 0) sid = sessions[0].id
  } catch { /* 无会话则走默认 */ }
  try {
    await sendMessage(text, setting.depth || 2, sid)
  } catch {
    askBusy.value = false
    askMsgs.value.push({ role: 'suzu', text: '（发送失败，请检查连接）' })
    scrollAskToBottom()
  }
}

// —— 展开态窗口失焦（点到窗口外）→ 自动收起，防尺寸残留 ——
async function onWinFocus(focused: boolean) {
  if (!focused && (panelOpen.value || askOpen.value) && !dragging) {
    await restoreBall()
    panelOpen.value = false
    askOpen.value = false
  }
}

// —— 尺寸自愈守卫：每 5s 校验一次窗口尺寸，防止意外变形残留（球被拉成椭圆）——
async function guardSize() {
  if (panelOpen.value || askOpen.value || dragging) return
  try {
    const outer = await win.outerSize()
    const monitor = (await currentMonitor()) || (await primaryMonitor())
    const scale = monitor?.scaleFactor || 1
    const w = Math.round(outer.width / scale)
    const h = Math.round(outer.height / scale)
    const expW = currentWinSize()
    const expH = mode.value === 'live2d' ? 400 : expW
    if (w !== expW || h !== expH) {
      await win.setSize(new LogicalSize(expW, expH))
    }
  } catch { /* 忽略 */ }
}

async function onContextMenu(e: MouseEvent) {
  e.preventDefault()
  if (!panelOpen.value) {
    openPanel()
  }
}

/** 面板项动作后统一收起 */
async function afterAction() {
  await closePanel()
}

async function menuShowMain() { await afterAction(); await showMain() }
/** 面板「快速提问」：直接切到迷你提问卡（不先收起再展开，避免闪烁） */
async function menuQuickAsk() {
  await afterAction()
  quickAsk()
}

// —— 小工具页：动态加载全部工具箱条目（预设 + 用户自定义 + 组合工具），悬浮球内不做编辑 ——
const toolsList = ref<{ id: string; label: string; steps: number }[]>([])
async function goTools() {
  panelPage.value = 'tools'
  // 打开时刷新（用户可能在设置页增删了工具）
  try {
    const res = await listToolboxItems()
    toolsList.value = res.items
      .filter((i) => i.enabled !== false)
      .map((i) => ({ id: i.id, label: i.name || i.id, steps: (i.steps?.length) || 0 }))
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

/** 切换显示模式（面板开着时仅存配置；窗口形态等收起后由 closePanel 统一处理） */
async function switchMode(m: 'avatar' | 'simple' | 'live2d') {
  if (mode.value === m) return
  try {
    await setting.update({ floating_ball_mode: m })
  } catch { /* 忽略 */ }
}

async function menuToggleMonitor() {
  const next = !monitorOn.value
  monitorOn.value = next
  try {
    await toggleMonitoring(next)
  } catch { /* 忽略 */ }
}

async function menuToggleClickThrough() {
  const next = !clickThrough.value
  try {
    const st = await setFloatingBallClickThrough(next)
    clickThrough.value = st
  } catch { /* 忽略 */ }
  if (clickThrough.value) {
    await afterAction()
    showNotice('鼠标穿透已开启，点系统托盘图标可关闭')
  } else {
    await afterAction()
  }
}

async function menuExit() {
  WebviewWindow.getByLabel('main').then((m) => m?.close())
}

// ==================== 小提示条（穿透/消息等临时提示，显示于球上方） ====================
const notice = ref('')
let noticeTimer: number | undefined
function showNotice(text: string) {
  notice.value = text
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = window.setTimeout(() => (notice.value = ''), 2600)
}

// ==================== 窗口事件监听 ====================
onMounted(async () => {
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)

  // 关键修复：悬浮球独立窗口此前从不加载配置 → 设置全部不生效。挂载即同步
  try {
    if (!setting.loaded) await setting.loadConfig()
  } catch { /* 忽略 */ }

  unlistenCfg = await onConfigUpdated(async () => {
    try {
      await setting.loadConfig()
    } catch { /* 忽略 */ }
  })

  unlistenCt = await onBallClickThroughChanged((st) => {
    clickThrough.value = st
  })

  // 面板展开时失焦自动收起 + 尺寸自愈定时器
  unlistenFocus = await win.onFocusChanged(({ payload: focused }) => onWinFocus(focused))
  sizeGuard = window.setInterval(() => guardSize(), 5000)

  // 托盘切换悬浮球显示/隐藏时：若被隐藏且面板还开着 → 收起面板复位（防"控制台残留"）
  unlistenToggle = await listen('floating-ball-toggled', async () => {
    try {
      const vis = await win.isVisible()
      if (!vis && (panelOpen.value || askOpen.value)) {
        await restoreBall()
        panelOpen.value = false
        askOpen.value = false
      }
    } catch { /* 忽略 */ }
  })

  // 全局热键 Ctrl+Alt+Q：Rust 已显示悬浮球，这里打开快速提问卡
  unlistenAskHotkey = await listen('ball-hotkey-ask', () => {
    if (clickThrough.value) return
    openAsk()
  })

  unlistenTrigger = await onMonitorTrigger(() => {
    if (!flash.value) return
    hasMessage.value = true
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = window.setTimeout(() => (hasMessage.value = false), 3000)
  })

  // 初始穿透状态 + 监测状态
  try { clickThrough.value = await getFloatingBallClickThrough() } catch { /* 忽略 */ }
  try {
    const res = await getMonitorRules()
    monitorOn.value = res.enabled
  } catch { /* 忽略 */ }

  // 初始：恢复位置 + 窗口大小
  try {
    const saved = localStorage.getItem(POS_KEY)
    if (saved) {
      const { x, y } = JSON.parse(saved)
      const [cx, cy] = await clampToScreen(x, y)
      await win.setPosition(new LogicalPosition(cx, cy))
    }
    const s = currentWinSize()
    await win.setSize(new LogicalSize(s, mode.value === 'live2d' ? 400 : s))
    if (mode.value === 'live2d') {
      await nextTick()
      await loadLive2D()
    }
  } catch { /* 忽略 */ }
})

onUnmounted(() => {
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
  if (flashTimer) clearTimeout(flashTimer)
  if (longTimer) clearTimeout(longTimer)
  if (cuddleTimer) clearTimeout(cuddleTimer)
  if (clickTimer) clearTimeout(clickTimer)
  if (popTimer) clearTimeout(popTimer)
  if (noticeTimer) clearTimeout(noticeTimer)
  if (sizeGuard) clearInterval(sizeGuard)
  if (snapRaf !== null) cancelAnimationFrame(snapRaf)
  unlistenTrigger?.()
  unlistenCfg?.()
  unlistenCt?.()
  unlistenFocus?.()
  unlistenToggle?.()
  unlistenAskHotkey?.()
  askUnlisten.forEach((fn) => { try { fn() } catch { /* 忽略 */ } })
  askUnlisten = []
  if (live2dCleanup) live2dCleanup()
})

// —— 模式变化：调整窗口大小 + 加载 Live2D ——
watch(mode, async (newMode) => {
  try {
    if (panelOpen.value) return // 面板展开中由 switchMode 处理
    if (newMode === 'live2d') {
      await win.setSize(new LogicalSize(300, 400))
      await nextTick()
      await loadLive2D()
    } else {
      const s = size.value
      await win.setSize(new LogicalSize(s, s))
    }
  } catch { /* 忽略 */ }
})

// —— 大小变化：头像/文字模式同步窗口 ——
watch(size, async (newSize) => {
  if (mode.value === 'live2d' || panelOpen.value) return
  try {
    await win.setSize(new LogicalSize(newSize, newSize))
  } catch { /* 忽略 */ }
})

// —— 开关变化：显示/隐藏 ——
watch(enabled, async (val) => {
  try {
    if (val) {
      await win.show()
      const saved = localStorage.getItem(POS_KEY)
      if (saved) {
        const { x, y } = JSON.parse(saved)
        const [cx, cy] = await clampToScreen(x, y)
        await win.setPosition(new LogicalPosition(cx, cy))
      }
    } else {
      await win.hide()
    }
  } catch { /* 忽略 */ }
})

// —— 外观：穿透时球体半透明化（不打扰画面），退出恢复 ——
const finalOpacity = computed(() => {
  if (clickThrough.value) return Math.min(opacity.value, 0.4)
  return opacity.value
})

// ==================== Live2D：本地加载内置模型（无需联网） ====================
const live2dMount = ref<HTMLDivElement | null>(null)
let live2dCleanup: (() => void) | null = null

async function loadLive2D() {
  if (mode.value !== 'live2d' || panelOpen.value || !live2dMount.value) return
  if (live2dCleanup) { live2dCleanup(); live2dCleanup = null }
  live2dMount.value.innerHTML = ''
  try {
    const { loadOml2d } = await import('oh-my-live2d')
    if (!live2dMount.value) return
    const container = live2dMount.value

    // 内置模型：开发/打包路径统一为 live2d/haru（相对资源目录）
    const dir = await resourceDir()
    const modelPath = `${dir}live2d/haru/haru_greeter_t03.model3.json`
    const modelUrl = convertFileSrc(modelPath)

    let oml2d: { onLoad: (fn: (s: string) => void) => void; destroy?: () => void } | null = null
    try {
      oml2d = loadOml2d({
        parentElement: container,
        // 纯模型展示：隐藏 oml2d 自带的菜单（书签按钮）与状态条（"看板娘休息中"等）
        statusBar: { disable: true },
        menus: { disable: true },
        models: [{ path: modelUrl, scale: 0.12 }],
      })
    } catch {
      // 初始化异常（老显卡 WebGL 兼容问题）→ 回退头像模式，避免渲染循环卡死
      showNotice('Live2D 不可用，已切换头像模式')
      setting.update({ floating_ball_mode: 'avatar' }).catch(() => {})
      return
    }
    oml2d?.onLoad((status) => {
      if (status === 'fail') {
        showNotice('Live2D 加载失败，已切换头像模式')
        setting.update({ floating_ball_mode: 'avatar' }).catch(() => {})
        try { oml2d?.destroy?.() } catch { /* 忽略 */ }
      }
    })
    live2dCleanup = () => {
      // 优先调用 oml2d 自毁（释放 pixi 渲染循环），失败则清 DOM 兜底
      try { oml2d?.destroy?.() } catch { /* 忽略 */ }
      if (live2dMount.value) live2dMount.value.innerHTML = ''
    }
  } catch {
    if (live2dMount.value) {
      live2dMount.value.innerHTML = '<div class="l2d-fail">Live2D 加载失败</div>'
    }
  }
}
</script>

<template>
  <div
    class="shell"
    :class="{ 'ct-on': clickThrough }"
    @mousedown="onMouseDown"
    @contextmenu="onContextMenu"
  >
    <!-- ====== 收起态：三种显示模式 ====== -->
    <template v-if="!panelOpen && !askOpen">
      <!-- 头像模式 -->
      <div
        v-if="mode === 'avatar'"
        class="ball avatar"
        :class="{ breathing: breathing && !hasMessage && !cuddling, flashing: flash && hasMessage, cuddling, popping }"
        :style="{ opacity: finalOpacity }"
      >
        <img :src="ballAvatar" class="avatar-img" draggable="false" alt="铃" />
      </div>

      <!-- 纯文字模式 -->
      <div
        v-else-if="mode === 'simple'"
        class="ball avatar"
        :class="{ breathing: breathing && !hasMessage && !cuddling, flashing: flash && hasMessage, cuddling, popping }"
        :style="{ opacity: finalOpacity }"
      >
        <span class="avatar-text">{{ setting.selfName || '铃' }}</span>
      </div>

      <!-- Live2D 模式 -->
      <div
        v-else-if="mode === 'live2d'"
        class="live2d-wrap"
        :class="{ flashing: flash && hasMessage }"
        :style="{ opacity: finalOpacity }"
      >
        <div ref="live2dMount" class="live2d-container"></div>
      </div>

      <!-- 消息角标（monitor 触发时右上角脉冲） -->
      <div v-if="hasMessage" class="msg-badge"></div>

      <!-- 穿透状态胶囊 -->
      <div v-if="clickThrough" class="ct-badge">穿透中 · 点托盘关闭</div>

      <!-- 临时提示条 -->
      <Transition name="fade">
        <div v-if="notice" class="notice-tip">{{ notice }}</div>
      </Transition>
    </template>

    <!-- ====== 展开态一：灵动控制面板（主菜单 / 小工具页） ====== -->
    <div v-else-if="panelOpen" class="panel">
      <div class="panel-head">
        <span class="mini-ball">
          <img v-if="mode === 'avatar'" :src="ballAvatar" class="avatar-img" draggable="false" />
          <span v-else-if="mode === 'simple'" class="mini-text">{{ setting.selfName || '铃' }}</span>
        </span>
        <span class="panel-title">{{ panelPage === 'tools' ? '小工具' : '铃 · 控制台' }}</span>
        <span class="panel-dot" :class="monitorOn ? 'on' : 'off'" :title="monitorOn ? '屏幕监测中' : '监测已暂停'"></span>
      </div>

      <div class="panel-body">
        <!-- 主菜单页 -->
        <template v-if="panelPage === 'main'">
          <div class="p-item" @click="menuShowMain">打开主窗口</div>
          <div class="p-item" @click="menuQuickAsk">快速提问</div>
          <div class="p-item" @click="goTools">工具箱<span class="p-hint">小工具 ›</span></div>

          <div class="p-sep"></div>

          <div class="p-seg-title">显示模式</div>
          <div class="p-seg">
            <div class="seg-btn" :class="{ active: mode === 'avatar' }" @click="switchMode('avatar')">头像</div>
            <div class="seg-btn" :class="{ active: mode === 'simple' }" @click="switchMode('simple')">文字</div>
            <div class="seg-btn" :class="{ active: mode === 'live2d' }" @click="switchMode('live2d')">Live2D</div>
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

        <!-- 小工具页 -->
        <template v-else>
          <div class="p-item" @click="panelPage = 'main'"><span class="back-arrow">‹</span> 返回主菜单</div>
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
      <!-- 对话记录 -->
      <div ref="askBodyRef" class="ask-body">
        <div v-if="askMsgs.length === 0 && !askStreaming" class="ask-empty">在这里直接和铃对话，回复实时显示～</div>
        <div
          v-for="(m, i) in askMsgs"
          :key="i"
          class="msg-line"
          :class="m.role === 'user' ? 'from-user' : 'from-suzu'"
        >{{ m.text }}</div>
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
          @keydown.enter.prevent="submitAsk"
        />
        <button class="ask-send" :disabled="askBusy || !askText.trim()" @click="submitAsk">发送</button>
      </div>
      <div class="ask-sub">对话会同步保存到最近会话</div>
    </div>
  </div>
</template>

<style scoped>
.shell {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  position: relative;
  user-select: none;
  -webkit-user-select: none;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-main, system-ui, 'Microsoft YaHei', sans-serif);
}

/* ===== 头像/文字模式球体（光晕随 size 参数化：外扩 ≤ 窗口余量，不被方框裁切） ===== */
.ball.avatar {
  width: 82%;
  height: 82%;
  --glow: calc(v-bind(size) * 0.09px);
  border-radius: 50%;
  background: linear-gradient(135deg, var(--accent, #ff7a94), color-mix(in srgb, var(--accent, #ff7a94) 55%, #6db3ff));
  border: 2px solid rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px calc(var(--glow) * 0.6) color-mix(in srgb, var(--accent, #ff7a94) 45%, transparent);
  cursor: grab;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  box-sizing: border-box;
  transition: box-shadow 0.2s;
}
.ball.avatar:active {
  cursor: grabbing;
  box-shadow: 0 6px var(--glow) color-mix(in srgb, var(--accent, #ff7a94) 65%, transparent);
}
/* 呼吸光晕（blur 上限 = --glow，外扩 < 窗口余量，圆形不削边） */
.ball.avatar.breathing {
  animation: breathe 3s ease-in-out infinite;
}
@keyframes breathe {
  0%, 100% { box-shadow: 0 4px calc(var(--glow) * 0.5) color-mix(in srgb, var(--accent, #ff7a94) 40%, transparent); }
  50% { box-shadow: 0 6px var(--glow) color-mix(in srgb, var(--accent, #ff7a94) 80%, transparent); }
}
/* 长按蹭蹭：轻微缩放起伏 */
.ball.avatar.cuddling {
  animation: cuddle 0.9s ease-in-out 1;
}
@keyframes cuddle {
  0%, 100% { transform: scale(1); }
  20% { transform: scale(0.93) rotate(-3deg); }
  45% { transform: scale(1.06) rotate(3deg); }
  70% { transform: scale(0.97) rotate(-2deg); }
}
/* 单击回弹 */
.ball.avatar.popping {
  animation: pop 0.32s ease-out 1;
}
@keyframes pop {
  0% { transform: scale(0.92); }
  55% { transform: scale(1.05); }
  100% { transform: scale(1); }
}

.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
  pointer-events: none;
}
.avatar-text {
  font-size: calc(v-bind(size) * 0.27px);
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
  pointer-events: none;
}

/* ===== Live2D ===== */
.live2d-wrap {
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.05);
  border-radius: 12px;
  overflow: hidden;
  cursor: grab;
  transition: transform 0.2s, box-shadow 0.2s;
}
.live2d-wrap:active { cursor: grabbing; transform: scale(1.01); }
.live2d-container { width: 100%; height: 100%; }
.l2d-fail {
  color: var(--text-secondary, #999);
  font-size: var(--fs-12, 12px);
  text-align: center;
  padding: 20px;
}

/* ===== 消息闪烁 ===== */
.ball.avatar.flashing,
.live2d-wrap.flashing {
  animation: flash 0.5s ease-in-out 6;
}
@keyframes flash {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}

/* ===== 消息角标 ===== */
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

/* ===== 穿透胶囊 ===== */
.ct-badge {
  position: absolute;
  top: 4px;
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

/* ===== 临时提示条 ===== */
.notice-tip {
  position: absolute;
  bottom: 10px;
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
.fade-enter-active, .fade-leave-active { transition: opacity 0.25s; }
.fade-enter-from, .fade-leave-to { opacity: 0; }

/* ===== 展开面板（灵动控制台） ===== */
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
.panel-dot.on { background: var(--success, #7fd99a); box-shadow: 0 0 6px var(--success, #7fd99a); }
.panel-dot.off { background: var(--text-secondary, #9a9294); }

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
.p-item.danger { color: var(--danger, #ff6b6b); }
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
.seg-btn:hover { color: var(--text-main, #eee6e7); }
.seg-btn.active {
  color: #fff;
  background: var(--accent, #ff7a94);
  border-color: var(--accent, #ff7a94);
  font-weight: 600;
}

/* ===== 小工具页 ===== */
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

/* ===== 快速提问迷你卡 ===== */
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
/* —— 迷你对话区 —— */
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
html, body, #app, .app-root {
  background: transparent !important;
  overflow: hidden !important;
  width: 100% !important;
  height: 100% !important;
  margin: 0 !important;
  padding: 0 !important;
}
/* 彻底隐藏悬浮球窗口的滚动条（滑轨） */
html::-webkit-scrollbar,
body::-webkit-scrollbar,
#app::-webkit-scrollbar,
*::-webkit-scrollbar {
  display: none !important;
  width: 0 !important;
  height: 0 !important;
}
</style>
