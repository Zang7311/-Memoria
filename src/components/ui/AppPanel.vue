<!-- 侧面板统一外壳（右侧抽屉用）：关闭按钮只有一处，标题字号统一 -->
<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string
    /** 是否显示关闭按钮 */
    closable?: boolean
  }>(),
  { closable: true },
)
const emit = defineEmits<{ close: [] }>()
</script>

<template>
  <div class="panel">
    <header class="panel-head">
      <span class="panel-title">{{ title }}</span>
      <div class="panel-extra">
        <slot name="actions" />
        <button v-if="closable" class="close" type="button" title="关闭" @click="emit('close')">
          <svg viewBox="0 0 24 24" class="ico" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      </div>
    </header>
    <div class="panel-body"><slot /></div>
    <footer v-if="$slots.footer" class="panel-foot"><slot name="footer" /></footer>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  background: var(--ui-panel);
  color: var(--ui-text);
}
.panel-head {
  height: var(--ui-head-h);
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: 0 var(--ui-sp-3);
  border-bottom: 1px solid var(--ui-border);
}
.panel-title {
  font-size: var(--ui-fs-14);
  font-weight: 600;
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.panel-extra {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  flex: none;
}
.close {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border: 0;
  background: transparent;
  border-radius: var(--ui-r-sm);
  color: var(--ui-text-2);
}
.close:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.ico {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
}
.panel-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--ui-sp-3);
}
.panel-foot {
  flex: none;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-top: 1px solid var(--ui-border);
}
</style>
