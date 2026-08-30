<!-- 《铃·记忆体》矢量图标组件（Phase2 SVG 替换 emoji）
     按当前主题风格自动选择图标目录：
       win10 / edge        -> fluent（Microsoft Fluent UI System Icons）
       ios-flat / ios-glass -> ios（Lucide 细线风格，SF-inspired）
       dark / light / minimal -> default（Tabler）
     渲染方式：v-html 内联 SVG 到 DOM，颜色通过 CSS 属性选择器控制：
       · 实心类（Fluent，无 fill="none"）-> fill: currentColor
       · 描边类（Lucide/Tabler，fill="none"）-> stroke: currentColor
     彻底解决 img/mask 方式下 currentColor 不继承/遮罩失效的问题 -->
<script setup lang="ts">
import { computed } from 'vue'
import { useSettingStore } from '../stores/settingStore'

const props = defineProps<{
  name: string
  size?: number | string
}>()

const setting = useSettingStore()

// 主题风格 -> 图标目录
function iconSet(theme: string): string {
  if (theme === 'win10' || theme === 'edge') return 'fluent'
  if (theme === 'ios-flat' || theme === 'ios-glass') return 'ios'
  return 'default' // dark / light / minimal / 其他
}

// 收集三套图标的 SVG 源码（?raw 拿到字符串，v-html 内联渲染）
const iconModules = import.meta.glob('../assets/icons/*/*.svg', { eager: true, query: '?raw', import: 'default' }) as Record<string, string>

const svg = computed<string>(() => {
  const set = iconSet(setting.theme)
  const key = `../assets/icons/${set}/${props.name}.svg`
  if (iconModules[key]) return iconModules[key]
  const fallback = `../assets/icons/default/${props.name}.svg`
  return iconModules[fallback] ?? ''
})

const style = computed(() => ({
  width: typeof props.size === 'number' ? `${props.size}px` : (props.size ?? '18px'),
  height: typeof props.size === 'number' ? `${props.size}px` : (props.size ?? '18px'),
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  flexShrink: 0,
  verticalAlign: '-0.15em',
}))
</script>

<template>
  <span v-if="svg" class="the-icon" :style="style" aria-hidden="true" v-html="svg" />
  <span v-else class="the-icon-missing" :style="style">▢</span>
</template>

<style scoped>
.the-icon {
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
  color: inherit; /* 跟随父元素文字色 */
}
/* SVG 铺满容器 */
.the-icon :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}
/* 实心类图标（Fluent：无 fill="none"）：用 fill 着色（path 继承） */
.the-icon :deep(svg:not([fill='none'])) {
  fill: currentColor;
}
/* 描边类图标（Lucide/Tabler：fill="none"）：用 stroke 着色，保留空心 */
.the-icon :deep(svg[fill='none']) {
  stroke: currentColor;
  stroke-width: 2;
}
/* 缺失兜底 */
.the-icon-missing {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0.5;
  font-size: 12px;
  color: var(--text-secondary, #999);
}
</style>
