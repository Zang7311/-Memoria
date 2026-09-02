<!-- 《铃·记忆体》悬浮球 v3（修复体验问题）
     修复：大小同步 / 拖拽瞬移 / 右键菜单被裁切 / 视觉溢出 -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  currentMonitor,
  getCurrentWindow,
  LogicalPosition,
  LogicalSize,
  primaryMonitor,
} from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { onMonitorTrigger } from '../utils/tauri'
import { useSettingStore } from '../stores/settingStore'

const win = getCurrentWindow()
const setting = useSettingStore()

// —— 响应式配置 ——
const mode = computed(() => setting.floatingBallMode)
const size = computed(() => setting.floatingBallSize)
const opacity = computed(() => setting.floatingBallOpacity)
const breathing = computed(() => setting.floatingBallBreathing)
const flash = computed(() => setting.floatingBallFlash)

// —— 拖拽状态 ——
const dragging = ref(false)
let dragStart = { x: 0, y: 0 }
let winStart = { x: 0, y: 0 }
let hasMoved = false

// —— 右键菜单 ——
const menuVisible = ref(false)
const menuPos = ref({ x: 0, y: 0 })

// —— 消息闪烁 ——
const hasMessage = ref(false)
let flashTimer: number | undefined
let unlistenTrigger: (() => void) | undefined

// —— 持久化位置 ——
const POS_KEY = 'floating-ball-pos-v3'
async function restorePosition() {
  try {
    const saved = localStorage.getItem(POS_KEY)
    if (saved) {
      const { x, y } = JSON.parse(saved)
      const monitor = (await currentMonitor()) || (await primaryMonitor())
      if (monitor) {
        const scale = monitor.scaleFactor || 1
        const lw = Math.round(monitor.size.width / scale)
        const lh = Math.round(monitor.size.height / scale)
        const s = size.value
        const cx = Math.min(Math.max(0, x), Math.max(0, lw - s))
        const cy = Math.min(Math.max(0, y), Math.max(0, lh - s))
        await win.setPosition(new LogicalPosition(cx, cy))
      }
    } else {
      // 默认右上角
      const monitor = (await currentMonitor()) || (await primaryMonitor())
      if (monitor) {
        const scale = monitor.scaleFactor || 1
        const lw = Math.round(monitor.size.width / scale)
        const s = size.value
        const x = Math.max(0, lw - s - 20)
        await win.setPosition(new LogicalPosition(x, 20))
      }
    }
    await win.setSize(new LogicalSize(size.value, size.value))
  } catch { /* 忽略 */ }
}

onMounted(async () => {
  await restorePosition()

  // 监听监测触发
  unlistenTrigger = await onMonitorTrigger(() => {
    if (!flash.value) return
    hasMessage.value = true
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = window.setTimeout(() => (hasMessage.value = false), 3000)
  })
})

onUnmounted(() => {
  if (flashTimer) clearTimeout(flashTimer)
  unlistenTrigger?.()
})

// —— 拖拽逻辑（修复瞬移：只在移动超过阈值后才更新位置） ——
async function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  dragging.value = true
  hasMoved = false
  dragStart = { x: e.screenX, y: e.screenY }
  try {
    const pos = await win.outerPosition()
    winStart = { x: pos.x, y: pos.y }
  } catch {
    winStart = { x: 0, y: 0 }
  }
}

async function onMouseMove(e: MouseEvent) {
  if (!dragging.value) return
  const dx = e.screenX - dragStart.x
  const dy = e.screenY - dragStart.y
  if (!hasMoved && Math.abs(dx) < 3 && Math.abs(dy) < 3) return
  hasMoved = true
  try {
    await win.setPosition(new LogicalPosition(winStart.x + dx, winStart.y + dy))
  } catch { /* 忽略 */ }
}

function onMouseUp() {
  if (!dragging.value) return
  dragging.value = false
  if (hasMoved) {
    win.outerPosition().then((p) => {
      try {
        localStorage.setItem(POS_KEY, JSON.stringify({ x: p.x, y: p.y }))
      } catch { /* 忽略 */ }
    })
  }
}

// —— 双击恢复主窗口 ——
async function onDoubleClick() {
  try {
    const main = await WebviewWindow.getByLabel('main')
    await main?.show()
    await main?.setFocus()
  } catch { /* 忽略 */ }
}

// —— 右键菜单 ——
function onContextMenu(e: MouseEvent) {
  e.preventDefault()
  // 菜单显示在鼠标位置，并确保不超出窗口
  const s = size.value
  const x = Math.min(e.clientX, s - 140)
  const y = Math.min(e.clientY, s - 100)
  menuPos.value = { x: Math.max(0, x), y: Math.max(0, y) }
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

// —— 窗口大小同步 ——
watch(size, async (newSize) => {
  try {
    await win.setSize(new LogicalSize(newSize, newSize))
  } catch { /* 忽略 */ }
})

// —— Live2D ——
const live2dMount = ref<HTMLDivElement | null>(null)
let live2dCleanup: (() => void) | null = null

const LIVE2D_MODELS = [
  { path: 'https://cdn.jsdelivr.net/gh/guansss/pixi-live2d-display/test/assets/haru/haru_greeter_t03.model3.json', name: 'Haru' },
  { path: 'https://cdn.jsdelivr.net/gh/guansss/pixi-live2d-display/test/assets/shizuku/shizuku.model.json', name: 'Shizuku' },
]

watch(mode, async (newMode) => {
  if (newMode !== 'live2d') {
    if (live2dCleanup) { live2dCleanup(); live2dCleanup = null }
    return
  }
  await loadLive2D()
})

async function loadLive2D() {
  if (mode.value !== 'live2d' || !live2dMount.value) return
  if (live2dCleanup) { live2dCleanup(); live2dCleanup = null }
  live2dMount.value.innerHTML = ''
  try {
    const { loadOml2d } = await import('oh-my-live2d')
    if (!live2dMount.value) return
    const container = live2dMount.value
    loadOml2d({
      parentElement: container,
      models: LIVE2D_MODELS.map(m => ({ path: m.path, scale: 0.25 })),
    })
    live2dCleanup = () => {
      if (live2dMount.value) live2dMount.value.innerHTML = ''
    }
  } catch (e) {
    console.warn('[Live2D] 加载失败：', e)
    if (live2dMount.value) {
      live2dMount.value.innerHTML = '<div style="color:#fff;font-size:12px;text-align:center;padding:8px;">Live2D 加载失败</div>'
    }
  }
}

onUnmounted(() => {
  if (live2dCleanup) live2dCleanup()
})
</script>

<template>
  <div
    class="ball-shell"
    @mousedown="onMouseDown"
    @mousemove="onMouseMove"
    @mouseup="onMouseUp"
    @mouseleave="onMouseUp"
    @dblclick="onDoubleClick"
    @contextmenu="onContextMenu"
  >
    <!-- 悬浮球主体：大小 100% 填满窗口，不再固定 72px -->
    <div
      class="ball"
      :class="{
        breathing: breathing && !dragging && !hasMessage,
        flashing: flash && hasMessage,
        dragging,
        'live2d-mode': mode === 'live2d'
      }"
      :style="{
        width: size + 'px',
        height: size + 'px',
        opacity: opacity,
        borderRadius: mode === 'live2d' ? '12px' : '50%',
      }"
    >
      <!-- 头像模式 -->
      <template v-if="mode === 'avatar'">
        <img
          v-if="setting.avatarSuzu && setting.avatarSuzu.startsWith('http')"
          :src="setting.avatarSuzu"
          class="ball-img"
          draggable="false"
        />
        <div v-else class="ball-text">{{ setting.selfName || '铃' }}</div>
      </template>

      <!-- 纯文字模式 -->
      <template v-else-if="mode === 'simple'">
        <div class="ball-text">{{ setting.selfName || '铃' }}</div>
      </template>

      <!-- Live2D 模式 -->
      <template v-else-if="mode === 'live2d'">
        <div ref="live2dMount" class="live2d-container"></div>
      </template>
    </div>

    <!-- 右键菜单（absolute 定位，跟随窗口，不被裁切） -->
    <div
      v-if="menuVisible"
      class="ball-menu"
      :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }"
      @click.stop
    >
      <div class="menu-item" @click="menuShowMain">显示主窗口</div>
      <div class="menu-item" @click="menuToggleMonitor">暂停/开启监测</div>
      <div class="menu-item danger" @click="menuExit">退出</div>
    </div>

    <!-- 遮罩 -->
    <div v-if="menuVisible" class="menu-mask" @click="hideMenu" @contextmenu.prevent="hideMenu" />
  </div>
</template>

<style scoped>
.ball-shell {
  width: 100vw;
  height: 100vh;
  overflow: visible;
  position: relative;
  user-select: none;
  -webkit-user-select: none;
}
.ball {
  /* 100% 填满窗口，不再固定 72px */
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, color-mix(in srgb, var(--accent, #ff7a94) 55%, #fff), color-mix(in srgb, var(--info, #6db3ff) 45%, #fff));
  border: 2px solid rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px 16px color-mix(in srgb, var(--accent, #ff7a94) 45%, transparent);
  cursor: grab;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
  overflow: hidden;
  box-sizing: border-box;
}
.ball.dragging {
  cursor: grabbing;
  transform: scale(1.05);
  box-shadow: 0 8px 24px rgba(255, 138, 171, 0.65);
}
.ball.breathing {
  animation: breathe 3s ease-in-out infinite;
}
@keyframes breathe {
  0%, 100% { transform: scale(1); box-shadow: 0 4px 16px rgba(255, 138, 171, 0.45); }
  50% { transform: scale(1.03); box-shadow: 0 6px 22px rgba(255, 138, 171, 0.65); }
}
.ball.flashing {
  animation: flash 0.5s ease-in-out 6;
  border-color: var(--accent);
}
@keyframes flash {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.35; transform: scale(1.08); }
}
.ball-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
  pointer-events: none;
}
.ball-text {
  font-size: calc(v-bind(size) * 0.25px);
  font-weight: 600;
  color: var(--text-user, #fff);
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
  pointer-events: none;
}
.live2d-container {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.1);
}
.ball-menu {
  position: absolute;
  z-index: 999;
  min-width: 130px;
  background: rgba(30, 28, 32, 0.96);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 10px;
  padding: 6px;
  backdrop-filter: blur(8px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
}
.menu-item {
  padding: 8px 12px;
  border-radius: 6px;
  font-size: var(--fs-13);
  color: var(--text-main, #eee6e7);
  cursor: pointer;
}
.menu-item:hover {
  background: rgba(255, 138, 171, 0.25);
}
.menu-item.danger {
  color: var(--accent);
}
.menu-mask {
  position: fixed;
  inset: 0;
  z-index: 998;
}
</style>

<style>
html,
body,
#app,
.app-root {
  background: transparent !important;
}
</style>
