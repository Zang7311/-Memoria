<!-- 全局消息提示宿主：放在主布局里渲染一次，任何地方 toast() 都显示在这里 -->
<script setup lang="ts">
import { toastState } from '../../utils/toast'
</script>

<template>
  <div class="toast-host" role="status" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="item in toastState.items" :key="item.id" class="toast" :class="`k-${item.kind}`">
        <span class="mark" aria-hidden="true" />
        <span class="text">{{ item.text }}</span>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: var(--ui-sp-4);
  bottom: var(--ui-sp-4);
  z-index: 700;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--ui-sp-2);
  pointer-events: none;
}
.toast {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-2);
  max-width: 420px;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-md);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
  line-height: var(--ui-line);
  box-shadow: var(--ui-shadow);
  pointer-events: auto;
}
.mark {
  width: 6px;
  height: 6px;
  flex: none;
  border-radius: var(--ui-r-pill);
  background: var(--ui-text-3);
}
.k-ok .mark {
  background: var(--ui-ok);
}
.k-err .mark {
  background: var(--ui-err);
}
.k-info .mark {
  background: var(--ui-accent);
}
.text {
  min-width: 0;
  overflow-wrap: anywhere;
}

/* 进场：右下滑入；出场：向右淡出（iOS 通知那种手感） */
.toast-enter-active,
.toast-leave-active {
  transition: opacity var(--ui-dur-2) var(--ui-ease), transform var(--ui-dur-2) var(--ui-ease);
}
.toast-enter-from {
  opacity: 0;
  transform: translateY(8px) scale(0.98);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(16px);
}
.toast-move {
  transition: transform var(--ui-dur-2) var(--ui-ease);
}
</style>
