<!-- 《铃·记忆体》悬浮球 v4（回归桌面宠物正常形态）
     头像模式：100x100 圆形
     Live2D 模式：300x400 竖屏矩形
     无滑块/无按钮组/无透明度调节 -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { currentMonitor, getCurrentWindow, LogicalPosition, LogicalSize, primaryMonitor } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { onMonitorTrigger } from '../utils/tauri'
import { useSettingStore } from '../stores/settingStore'

const win = getCurrentWindow()
const setting = useSettingStore()
const mode = computed(() => setting.floatingBallMode)
const enabled = computed(() => setting.floatingBallEnabled)
const hasMessage = ref(false)
let flashTimer: number | undefined
let unlistenTrigger: (() => void) | undefined

// —— 拖拽状态 ——
let dragging = false
let startMouse = { x: 0, y: 0 }
let startPos = { x: 0, y: 0 }

// —— 持久化位置 ——
const POS_KEY = 'floating-ball-pos-v4'

async function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  dragging = true
  startMouse = { x: e.screenX, y: e.screenY }
  try {
    const pos = await win.outerPosition()
    startPos = { x: pos.x, y: pos.y }
  } catch {
    startPos = { x: 0, y: 0 }
  }
}

async function onMouseMove(e: MouseEvent) {
  if (!dragging) return
  const dx = e.screenX - startMouse.x
  const dy = e.screenY - startMouse.y
  try {
    await win.setPosition(new LogicalPosition(startPos.x + dx, startPos.y + dy))
  } catch { /* 忽略 */ }
}

function onMouseUp() {
  if (!dragging) return
  dragging = false
}

// —— 双击打开主窗口 ——
async function onDoubleClick() {
  try {
    const main = await WebviewWindow.getByLabel('main')
    await main?.show()
    await main?.setFocus()
  } catch { /* 忽略 */ }
}

// —— 右键菜单（简洁，不遮罩） ——
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

// —— 消息闪烁 ——
onMounted(async () => {
  unlistenTrigger = await onMonitorTrigger(() => {
    hasMessage.value = true
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = window.setTimeout(() => (hasMessage.value = false), 3000)
  })
})

onUnmounted(() => {
  if (flashTimer) clearTimeout(flashTimer)
  unlistenTrigger?.()
})

// —— 监听模式变化，自动调整窗口大小 ——
watch(mode, async (newMode) => {
  try {
    if (newMode === 'live2d') {
      await win.setSize(new LogicalSize(300, 400))
    } else {
      await win.setSize(new LogicalSize(100, 100))
    }
  } catch { /* 忽略 */ }
})

// —— 监听开关变化，自动显示/隐藏窗口 ——
watch(enabled, async (val) => {
  try {
    if (val) {
      await win.show()
      // 恢复位置
      const saved = localStorage.getItem(POS_KEY)
      if (saved) {
        const { x, y } = JSON.parse(saved)
        const monitor = (await currentMonitor()) || (await primaryMonitor())
        if (monitor) {
          const scale = monitor.scaleFactor || 1
          const lw = Math.round(monitor.size.width / scale)
          const lh = Math.round(monitor.size.height / scale)
          const s = mode.value === 'live2d' ? 300 : 100
          const cx = Math.min(Math.max(0, x), Math.max(0, lw - s))
          const cy = Math.min(Math.max(0, y), Math.max(0, lh - s))
          await win.setPosition(new LogicalPosition(cx, cy))
        }
      }
    } else {
      await win.hide()
    }
  } catch { /* 忽略 */ }
})
</script>

<template>
  <div
    class="shell"
    @mousedown="onMouseDown"
    @mousemove="onMouseMove"
    @mouseup="onMouseUp"
    @mouseleave="onMouseUp"
    @dblclick="onDoubleClick"
    @contextmenu="onContextMenu"
  >
    <!-- 头像模式：100x100 圆形 -->
    <template v-if="mode === 'avatar' || mode === 'simple'">
      <div class="ball avatar" :class="{ flashing: hasMessage }">
        <img
          v-if="setting.avatarSuzu && setting.avatarSuzu.startsWith('http')"
          :src="setting.avatarSuzu"
          class="avatar-img"
          draggable="false"
        />
        <span v-else class="avatar-text">{{ setting.selfName || '铃' }}</span>
      </div>
    </template>

    <!-- Live2D 模式：300x400 矩形 -->
    <template v-else-if="mode === 'live2d'">
      <div class="live2d-wrap" :class="{ flashing: hasMessage }">
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
  overflow: visible;
  position: relative;
  user-select: none;
  -webkit-user-select: none;
}

/* 头像模式：固定 100x100 圆形 */
.ball.avatar {
  width: 100px;
  height: 100px;
  border-radius: 50%;
  background: linear-gradient(135deg, #ff7a94, #6db3ff);
  border: 2px solid rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px 16px rgba(255, 138, 171, 0.45);
  cursor: grab;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  transition: transform 0.2s, box-shadow 0.2s;
}
.ball.avatar:active {
  cursor: grabbing;
  transform: scale(1.08);
  box-shadow: 0 8px 24px rgba(255, 138, 171, 0.65);
}

.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
  pointer-events: none;
}
.avatar-text {
  font-size: 32px;
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
  pointer-events: none;
}

/* Live2D 模式：300x400 矩形 */
.live2d-wrap {
  width: 300px;
  height: 400px;
  background: rgba(0, 0, 0, 0.05);
  border-radius: 12px;
  overflow: hidden;
  cursor: grab;
  transition: transform 0.2s, box-shadow 0.2s;
}
.live2d-wrap:active {
  cursor: grabbing;
  transform: scale(1.02);
}
.live2d-wrap.flashing {
  animation: flash 0.5s ease-in-out 6;
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
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(1.06); }
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
}
</style>
