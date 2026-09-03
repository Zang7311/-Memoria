<!-- 《铃·记忆体》悬浮球 v6
     修复：DPi 拖拽抖动（物理像素÷缩放+rAF）/ 呼吸不光晕不削边 / Live2D 本地加载不联网 -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { resourceDir } from '@tauri-apps/api/path'
import { currentMonitor, getCurrentWindow, LogicalPosition, LogicalSize, primaryMonitor } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { onMonitorTrigger } from '../utils/tauri'
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
let flashTimer: number | undefined
let unlistenTrigger: (() => void) | undefined

// —— 拖拽（物理像素差值 ÷ 缩放比 = 逻辑坐标，rAF 节流防乱序） ——
let dragging = false
let dragReady = false
let moved = false
let peakDist = 0
let startScreen = { x: 0, y: 0 }
let startPos = { x: 0, y: 0 }
let dpiScale = 1
let targetPos = { x: 0, y: 0 }
let rafId: number | null = null

// —— 位置持久化 ——
const POS_KEY = 'floating-ball-pos-v6'

/** 当前模式的窗口尺寸 */
function currentWinSize(): number {
  return mode.value === 'live2d' ? 300 : size.value
}

/** 把位置约束在当前屏幕内 */
async function clampToScreen(x: number, y: number): Promise<[number, number]> {
  try {
    const monitor = (await currentMonitor()) || (await primaryMonitor())
    if (monitor) {
      const scale = monitor.scaleFactor || 1
      const lw = Math.round(monitor.size.width / scale)
      const lh = Math.round(monitor.size.height / scale)
      const s = currentWinSize()
      return [Math.min(Math.max(0, x), Math.max(0, lw - s)), Math.min(Math.max(0, y), Math.max(0, lh - s))]
    }
  } catch { /* 忽略 */ }
  return [x, y]
}

async function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  e.preventDefault()
  dragging = true
  moved = false
  peakDist = 0
  dragReady = false
  startScreen = { x: e.screenX, y: e.screenY }
  try {
    const pos = await win.outerPosition()
    startPos = { x: pos.x, y: pos.y }
    const mon = (await currentMonitor()) || (await primaryMonitor())
    dpiScale = mon?.scaleFactor || 1
  } catch {
    startPos = { x: 0, y: 0 }
  } finally {
    dragReady = true
  }
}

function onMouseMove(e: MouseEvent) {
  if (!dragging || !dragReady) return
  const dx = (e.screenX - startScreen.x) / dpiScale
  const dy = (e.screenY - startScreen.y) / dpiScale
  const dist = Math.hypot(dx, dy)
  if (dist > peakDist) peakDist = dist
  // 移动超过 3px 才算拖动；纯点击（手抖）不移动窗口
  if (!moved && dist < 3) return
  moved = true
  targetPos.x = startPos.x + dx
  targetPos.y = startPos.y + dy
  if (rafId !== null) return
  rafId = window.requestAnimationFrame(() => {
    rafId = null
    win.setPosition(new LogicalPosition(Math.round(targetPos.x), Math.round(targetPos.y))).catch(() => {})
  })
}

function onMouseUp() {
  if (!dragging) return
  dragging = false
  if (rafId !== null) {
    window.cancelAnimationFrame(rafId)
    rafId = null
  }
  // 峰值位移 < 8px 视为"点击"：把窗口吸回按下时的位置（消除手抖漂移），不保存
  if (!moved || peakDist < 8) {
    win.setPosition(new LogicalPosition(Math.round(startPos.x), Math.round(startPos.y))).catch(() => {})
    return
  }
  // 真拖动：保存当前位置
  win.outerPosition().then((p) => {
    try {
      localStorage.setItem(POS_KEY, JSON.stringify({ x: p.x, y: p.y }))
    } catch { /* 忽略 */ }
  }).catch(() => {})
}

// —— 双击打开主窗口 ——
async function onDoubleClick() {
  try {
    const main = await WebviewWindow.getByLabel('main')
    await main?.show()
    await main?.setFocus()
  } catch { /* 忽略 */ }
}

// —— 右键菜单 ——
const menuVisible = ref(false)
const menuPos = ref({ x: 0, y: 0 })

function onContextMenu(e: MouseEvent) {
  e.preventDefault()
  menuPos.value = { x: e.clientX, y: e.clientY }
  menuVisible.value = true
}

function hideMenu() {
  menuVisible.value = false
}

async function menuShowMain() {
  hideMenu()
  await onDoubleClick()
}

async function menuToggleMonitor() {
  hideMenu()
  try {
    const res = await (await import('../utils/tauri')).getMonitorRules()
    await (await import('../utils/tauri')).toggleMonitoring(!res.enabled)
  } catch { /* 忽略 */ }
}

function menuExit() {
  hideMenu()
  WebviewWindow.getByLabel('main').then((m) => m?.close())
}

// —— 窗口事件监听（window 级，拖动流畅） ——
onMounted(async () => {
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)

  unlistenTrigger = await onMonitorTrigger(() => {
    if (!flash.value) return
    hasMessage.value = true
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = window.setTimeout(() => (hasMessage.value = false), 3000)
  })

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
  } catch { /* 忽略 */ }
})

onUnmounted(() => {
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
  if (flashTimer) clearTimeout(flashTimer)
  unlistenTrigger?.()
  if (live2dCleanup) live2dCleanup()
})

// —— 模式变化：调整窗口大小 + 加载 Live2D ——
watch(mode, async (newMode) => {
  try {
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

// —— 大小变化：头像模式同步窗口 ——
watch(size, async (newSize) => {
  if (mode.value === 'live2d') return
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

// —— Live2D：本地加载内置模型（无需联网） ——
const live2dMount = ref<HTMLDivElement | null>(null)
let live2dCleanup: (() => void) | null = null

async function loadLive2D() {
  if (mode.value !== 'live2d' || !live2dMount.value) return
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
    console.warn('[Live2D] 模型路径：', modelPath)

    const oml2d = loadOml2d({
      parentElement: container,
      models: [{ path: modelUrl, scale: 0.12 }],
    })
    oml2d.onLoad((status) => {
      console.warn('[Live2D] 加载状态：', status)
      if (status === 'fail') {
        container.innerHTML = '<div style="color:#fff;font-size:12px;text-align:center;padding:20px;">Live2D 加载失败</div>'
      }
    })
    live2dCleanup = () => {
      if (live2dMount.value) live2dMount.value.innerHTML = ''
    }
  } catch (e) {
    console.warn('[Live2D] 加载异常：', e)
    if (live2dMount.value) {
      live2dMount.value.innerHTML = '<div style="color:#fff;font-size:12px;text-align:center;padding:20px;">Live2D 加载失败</div>'
    }
  }
}
</script>

<template>
  <div
    class="shell"
    @mousedown="onMouseDown"
    @dblclick="onDoubleClick"
    @contextmenu="onContextMenu"
  >
    <!-- 头像模式 -->
    <template v-if="mode === 'avatar'">
      <div
        class="ball avatar"
        :class="{ breathing: breathing && !hasMessage, flashing: flash && hasMessage }"
        :style="{ opacity }"
      >
        <img :src="ballAvatar" class="avatar-img" draggable="false" alt="铃" />
      </div>
    </template>

    <!-- 纯文字模式 -->
    <template v-else-if="mode === 'simple'">
      <div
        class="ball avatar"
        :class="{ breathing: breathing && !hasMessage, flashing: flash && hasMessage }"
        :style="{ opacity }"
      >
        <span class="avatar-text">{{ setting.selfName || '铃' }}</span>
      </div>
    </template>

    <!-- Live2D 模式 -->
    <template v-else-if="mode === 'live2d'">
      <div class="live2d-wrap" :class="{ flashing: flash && hasMessage }" :style="{ opacity }">
        <div ref="live2dMount" class="live2d-container"></div>
      </div>
    </template>

    <!-- 右键菜单 -->
    <div
      v-if="menuVisible"
      class="menu"
      :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }"
      @click.stop
    >
      <div class="menu-item" @click="menuShowMain">打开主窗口</div>
      <div class="menu-item" @click="menuToggleMonitor">暂停监测</div>
      <div class="menu-item danger" @click="menuExit">退出</div>
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
}

/* 头像/文字模式：球占外框 90%（直径小于外框边长，任何缩放都不削边） */
.ball.avatar {
  width: 90%;
  height: 90%;
  border-radius: 50%;
  background: linear-gradient(135deg, #ff7a94, #6db3ff);
  border: 2px solid rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px 16px rgba(255, 138, 171, 0.45);
  cursor: grab;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  box-sizing: border-box;
  transition: box-shadow 0.2s;
}
/* 点击/拖动：只加深阴影，不缩放（球不会"动"） */
.ball.avatar:active {
  cursor: grabbing;
  box-shadow: 0 8px 24px rgba(255, 138, 171, 0.65);
}
/* 呼吸：仅光晕变化，不改变大小（不削边） */
.ball.avatar.breathing {
  animation: breathe 3s ease-in-out infinite;
}
@keyframes breathe {
  0%, 100% { box-shadow: 0 4px 14px rgba(255, 138, 171, 0.4); }
  50% { box-shadow: 0 7px 26px rgba(255, 138, 171, 0.8); }
}

.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
  pointer-events: none;
}
.avatar-text {
  font-size: calc(v-bind(size) * 0.32px);
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
  pointer-events: none;
}

/* Live2D 模式 */
.live2d-wrap {
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.05);
  border-radius: 12px;
  overflow: hidden;
  cursor: grab;
  transition: transform 0.2s, box-shadow 0.2s;
}
.live2d-wrap:active {
  cursor: grabbing;
  transform: scale(1.01);
}
.live2d-container {
  width: 100%;
  height: 100%;
}

/* 闪烁动画 */
.ball.avatar.flashing,
.live2d-wrap.flashing {
  animation: flash 0.5s ease-in-out 6;
}
@keyframes flash {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

/* 右键菜单 */
.menu {
  position: absolute;
  z-index: 999;
  min-width: 120px;
  background: rgba(30, 28, 32, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  padding: 4px;
  backdrop-filter: blur(8px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
}
.menu-item {
  padding: 7px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: #eee6e7;
  cursor: pointer;
  white-space: nowrap;
}
.menu-item:hover {
  background: rgba(255, 138, 171, 0.25);
}
.menu-item.danger {
  color: #ff7a94;
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