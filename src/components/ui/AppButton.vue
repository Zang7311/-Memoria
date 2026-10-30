<!-- 统一按钮：三档变体，只有 primary 用主色（一屏最多一个 primary） -->
<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: 'primary' | 'ghost' | 'text'
    size?: 'sm' | 'md'
    disabled?: boolean
    type?: 'button' | 'submit'
    block?: boolean
  }>(),
  { variant: 'ghost', size: 'md', disabled: false, type: 'button', block: false },
)
</script>

<template>
  <button
    :type="type"
    class="app-btn"
    :class="[`is-${variant}`, `s-${size}`, { block }]"
    :disabled="disabled"
  >
    <slot />
  </button>
</template>

<style scoped>
.app-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--ui-sp-1);
  min-width: 0;
  border: 1px solid transparent;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-13);
  line-height: 1;
  white-space: nowrap;
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease),
    border-color var(--ui-dur-1) var(--ui-ease);
}
.s-md {
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
}
.s-sm {
  height: 26px;
  padding: 0 var(--ui-sp-2);
  font-size: var(--ui-fs-12);
}
.block {
  width: 100%;
}

/* 线框按钮：默认档，大多数操作都用它 */
.is-ghost {
  border-color: var(--ui-border);
  background: var(--ui-raised);
  color: var(--ui-text);
}
.is-ghost:hover:not(:disabled) {
  background: var(--ui-hover);
  border-color: var(--ui-border-strong);
}

/* 主操作：粉底，一屏最多一个 */
.is-primary {
  background: var(--ui-accent);
  color: #fff;
}
.is-primary:hover:not(:disabled) {
  filter: brightness(1.06);
}

/* 文字按钮：次要操作、面板内的小动作 */
.is-text {
  color: var(--ui-text-2);
}
.is-text:hover:not(:disabled) {
  background: var(--ui-hover);
  color: var(--ui-text);
}

.app-btn:active:not(:disabled) {
  transform: translateY(0.5px);
}
</style>
