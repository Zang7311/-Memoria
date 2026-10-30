<!-- 全局确认框：配合 utils/dialog.ts 的 confirmDialog() 使用，主布局里渲染一次 -->
<script setup lang="ts">
import { dialogState, resolveDialog } from '../../utils/dialog'
</script>

<template>
  <Transition name="dlg">
    <div v-if="dialogState.open" class="mask" @click.self="resolveDialog(false)">
      <div class="dialog" role="dialog" aria-modal="true">
        <p class="title">{{ dialogState.title }}</p>
        <p class="text">{{ dialogState.text }}</p>
        <div class="foot">
          <button class="btn" type="button" @click="resolveDialog(false)">
            {{ dialogState.cancelText }}
          </button>
          <button
            class="btn primary"
            :class="{ danger: dialogState.danger }"
            type="button"
            @click="resolveDialog(true)"
          >
            {{ dialogState.confirmText }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.mask {
  position: fixed;
  inset: 0;
  z-index: 750;
  display: grid;
  place-items: center;
  background: color-mix(in srgb, var(--ui-bg) 55%, transparent);
  backdrop-filter: blur(3px);
}
.dialog {
  width: min(380px, calc(100vw - 48px));
  padding: var(--ui-sp-4);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-lg);
  background: var(--ui-raised);
  box-shadow: var(--ui-shadow);
}
.title {
  margin: 0 0 var(--ui-sp-2);
  font-size: var(--ui-fs-14);
  color: var(--ui-text);
}
.text {
  margin: 0 0 var(--ui-sp-4);
  font-size: var(--ui-fs-13);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
  overflow-wrap: anywhere;
}
.foot {
  display: flex;
  justify-content: flex-end;
  gap: var(--ui-sp-2);
}
.btn {
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-4);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-13);
}
.btn:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.btn.primary {
  border-color: var(--ui-accent);
  background: var(--ui-accent);
  color: var(--ui-on-accent);
}
.btn.primary.danger {
  border-color: var(--ui-err);
  background: var(--ui-err);
}
.dlg-enter-active,
.dlg-leave-active {
  transition: opacity var(--ui-dur-1) var(--ui-ease);
}
.dlg-enter-active .dialog,
.dlg-leave-active .dialog {
  transition: transform var(--ui-dur-2) var(--ui-ease);
}
.dlg-enter-from,
.dlg-leave-to {
  opacity: 0;
}
.dlg-enter-from .dialog,
.dlg-leave-to .dialog {
  transform: scale(0.96);
}
</style>
