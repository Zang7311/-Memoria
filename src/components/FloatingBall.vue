<!-- 《铃·记忆体》悬浮球 v2（全面改进）
     三种模式：avatar / simple / live2d
     支持大小/透明度/动画自定义 -->
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
import { onMonitorTrigger, toggleMonitoring, getMonitorRules } from '../utils/tauri'
import { useSettingStore } from '../stores/settingStore'

const win = getCurrentWindow()
const setting = useSettingStore()

// —— 模式切换监听 ——
const mode = computed(() => setting.floatingBallMode)
const size = computed(() => setting.floatingBallSize)
const opacity = computed(() => setting.floatingBallOpacity)
const breathing = computed(() => setting.floatingBallBreathing)
const flash = computed(() => setting.floatingBallFlash)

// —— 拖拽 ——
const dragging = ref(false)
let startMouse = { x: 0, y: 0 }
let startPos = { x: 0, y: 0 }

// —— 右键菜单 ——
const menuVisible = ref(false)
const menuPos = ref({ x: 0, y: 0 })

// —— 消息闪烁 ——
const hasMessage = ref(false)
let flashTimer: number | undefined
let unlistenTrigger: (() => void) | undefined

// —— 位置持久化 ——
const POS_KEY = 'floating-ball-pos-v2'
onMounted(async () => {
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
        await win.setSize(new LogicalSize(s, s))
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
        await win.setSize(new LogicalSize(s, s))
      }
    }
  } catch { /* 忽略 */ }

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

// —— 拖拽逻辑 ——
async function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  dragging.value = true
  startMouse = { x: e.screenX, y: e.screenY }
  try {
    const pos = await win.outerPosition()
    startPos = { x: pos.x, y: pos.y }
  } catch {
    startPos = { x: 0, y: 0 }
  }
}

async function onMouseMove(e: MouseEvent) {
  if (!dragging.value) return
  const nx = startPos.x + (e.screenX - startMouse.x)
  const ny = startPos.y + (e.screenY - startMouse.y)
  try {
    const s = size.value
    await win.setPosition(new LogicalPosition(nx, ny))
    await win.setSize(new LogicalSize(s, s))
  } catch { /* 忽略 */ }
}

function onMouseUp() {
  if (!dragging.value) return
  dragging.value = false
  win.outerPosition().then((p) => {
    try {
      localStorage.setItem(POS_KEY, JSON.stringify({ x: p.x, y: p.y }))
    } catch { /* 忽略 */ }
  })
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
    const res = await getMonitorRules()
    await toggleMonitoring(!res.enabled)
  } catch { /* 忽略 */ }
}

function menuExit() {
  hideMenu()
  WebviewWindow.getByLabel('main').then((m) => m?.close())
}

// —— 窗口大小同步（当设置改变时） ——
watch([size, opacity], async ([newSize]) => {
  try {
    await win.setSize(new LogicalSize(newSize, newSize))
  } catch { /* 忽略 */ }
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
    <!-- 悬浮球主体 -->
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
        <div class="live2d-container">
          <div class="live2d-placeholder">Live2D</div>
        </div>
      </template>
    </div>

    <!-- 右键菜单 -->
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
  overflow: hidden;
  position: relative;
  user-select: none;
  -webkit-user-select: none;
}
.ball {
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, color-mix(in srgb, var(--accent, #ff7a94) 55%, #fff), color-mix(in srgb, var(--info, #6db3ff) 45%, #fff));
  border: 2px solid rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px 16px color-mix(in srgb, var(--accent, #ff7a94) 45%, transparent);
  cursor: grab;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
  overflow: hidden;
}
.ball.dragging {
  cursor: grabbing;
  transform: scale(1.1);
  box-shadow: 0 8px 24px rgba(255, 138, 171, 0.65);
}
.ball.breathing {
  animation: breathe 3s ease-in-out infinite;
}
@keyframes breathe {
  0%, 100% { transform: scale(1); box-shadow: 0 4px 16px rgba(255, 138, 171, 0.45); }
  50% { transform: scale(1.06); box-shadow: 0 6px 22px rgba(255, 138, 171, 0.65); }
}
.ball.flashing {
  animation: flash 0.5s ease-in-out 6;
  border-color: var(--accent);
}
@keyframes flash {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.35; transform: scale(1.15); }
}
.ball-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
  pointer-events: none;
}
.ball-text {
  font-size: calc(v-bind(size) * 0.4px);
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
.live2d-placeholder {
  color: var(--text-user, #fff);
  font-size: calc(v-bind(size) * 0.15px);
  opacity: 0.8;
}
.ball-menu {
  position: fixed;
  z-index: 999;
  min-width: 130px;
  background: rgba(30, 28, 32, 0.95);
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
