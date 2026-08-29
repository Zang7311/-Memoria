<!-- 《铃·记忆体》像素画板（批次3）：32×32 网格画板，选色绘制/右键擦除，保存 PNG 到桌面 -->
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { savePixelArt } from '../utils/tauri'

const emit = defineEmits<{ close: [] }>()
const SIZE = 32
const canvas = ref<HTMLCanvasElement | null>(null)
const colors = ['#000000', '#ffffff', '#ff0000', '#ff7700', '#ffff00', '#00ff00', '#00ff88', '#00ffff', '#0088ff', '#0000ff', '#8800ff', '#ff00ff', '#ff0088', '#ff7a94', '#ffd700', '#c0c0c0', '#a0a0a0', '#665544', '#553300', '#332200']
const curColor = ref('#ff7a94')
const msg = ref('')
let drawing = false
let erasing = false

function ctx2d() {
  return canvas.value?.getContext('2d')
}
function getPixel(e: MouseEvent) {
  const cv = canvas.value!
  const r = cv.getBoundingClientRect()
  const x = Math.floor(((e.clientX - r.left) / r.width) * SIZE)
  const y = Math.floor(((e.clientY - r.top) / r.height) * SIZE)
  return { x: Math.max(0, Math.min(SIZE - 1, x)), y: Math.max(0, Math.min(SIZE - 1, y)) }
}
function paint(x: number, y: number) {
  const c = ctx2d()!
  c.fillStyle = erasing ? '#ffffff' : curColor.value
  c.fillRect(x, y, 1, 1)
}
function onDown(e: MouseEvent) {
  e.preventDefault()
  erasing = e.button === 2
  drawing = true
  const { x, y } = getPixel(e)
  paint(x, y)
}
function onMove(e: MouseEvent) {
  if (!drawing) return
  const { x, y } = getPixel(e)
  paint(x, y)
}
function onUp() {
  drawing = false
}
function onContext(e: MouseEvent) {
  e.preventDefault()
}
function clear() {
  ctx2d()!.clearRect(0, 0, SIZE, SIZE)
  ctx2d()!.fillStyle = '#ffffff'
  ctx2d()!.fillRect(0, 0, SIZE, SIZE)
}
async function save() {
  if (!canvas.value) return
  try {
    const path = await savePixelArt(canvas.value.toDataURL('image/png'))
    msg.value = `已保存到桌面：${path.split('\\').pop()}`
  } catch (e) {
    msg.value = `保存失败：${e}`
  }
}
onMounted(clear)
</script>

<template>
  <div class="pixel-panel" @click.self="emit('close')">
    <div class="pp-head">
      <span class="pp-title">像素画板（32×32）</span>
      <div class="pp-btns">
        <button class="btn ghost" @click="clear">清空</button>
        <button class="btn primary" @click="save">保存 PNG</button>
        <button class="btn ghost" @click="emit('close')">关闭</button>
      </div>
    </div>
    <div class="pp-colors">
      <button
        v-for="c in colors"
        :key="c"
        class="pp-color"
        :class="{ sel: c === curColor }"
        :style="{ background: c }"
        @click="curColor = c"
      />
      <input v-model="curColor" type="color" class="pp-color-custom" title="自定义颜色" />
    </div>
    <canvas
      ref="canvas"
      :width="SIZE"
      :height="SIZE"
      @mousedown="onDown"
      @mousemove="onMove"
      @mouseup="onUp"
      @mouseleave="onUp"
      @contextmenu="onContext"
    />
    <p class="pp-msg">左键绘制 · 右键擦除 · {{ msg }}</p>
  </div>
</template>

<style scoped>
.pixel-panel {
  position: fixed;
  inset: 0;
  z-index: 600;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 10px;
  padding: 20px 0;
  box-sizing: border-box;
  overflow-y: auto;
}
.pp-head {
  background: var(--bg-bar, rgba(34, 32, 36, 0.92));
  padding: 8px 14px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 420px;
  flex-shrink: 0;
}
.pp-title { font-weight: 600; color: var(--text-main, #eee); white-space: nowrap; font-size: 14px; }
.pp-colors { display: flex; gap: 5px; flex-wrap: wrap; width: 420px; flex-shrink: 0; }
.pp-color { width: 24px; height: 24px; border-radius: 6px; border: 2px solid rgba(128, 128, 128, 0.4); cursor: pointer; }
.pp-color.sel { border-color: var(--accent, #ff7a94); transform: scale(1.15); }
.pp-color-custom { width: 24px; height: 24px; border-radius: 6px; border: 2px solid rgba(128, 128, 128, 0.4); cursor: pointer; padding: 0; background: transparent; }
.pp-btns { display: flex; gap: 6px; }
canvas {
  image-rendering: pixelated;
  width: 420px;
  height: 420px;
  background: #fff;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.2));
  cursor: crosshair;
  flex-shrink: 0;
}
.pp-msg { color: var(--text-main, #eee); font-size: 12px; flex-shrink: 0; }
</style>
