<!-- 统一开关：左文字右开关，整行可点；不使用 emoji 表示状态 -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    modelValue: boolean
    label?: string
    hint?: string
    disabled?: boolean
  }>(),
  { label: '', hint: '', disabled: false },
)
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

function toggle() {
  if (props.disabled) return
  emit('update:modelValue', !props.modelValue)
}
</script>

<template>
  <label class="sw" :class="{ disabled }">
    <span class="text">
      <span class="label">{{ label }}<slot /></span>
      <span v-if="hint" class="hint">{{ hint }}</span>
    </span>
    <input
      class="native"
      type="checkbox"
      :checked="modelValue"
      :disabled="disabled"
      @change="toggle"
    />
    <span class="track" :class="{ on: modelValue }" aria-hidden="true">
      <span class="knob" />
    </span>
  </label>
</template>

<style scoped>
.sw {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-3);
  padding: var(--ui-sp-2) 0;
  cursor: pointer;
  user-select: none;
}
.sw.disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}
.label {
  font-size: var(--ui-fs-13);
  color: var(--ui-text);
  line-height: var(--ui-line);
}
.hint {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
/* 原生 checkbox 隐藏但保留键盘可达 */
.native {
  position: absolute;
  opacity: 0;
  width: 1px;
  height: 1px;
  pointer-events: none;
}
.track {
  flex: none;
  width: 36px;
  height: 20px;
  border-radius: var(--ui-r-pill);
  background: var(--ui-active);
  border: 1px solid var(--ui-border);
  transition: background var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease);
  position: relative;
}
.track.on {
  background: var(--ui-accent);
  border-color: var(--ui-accent);
}
.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: var(--ui-r-pill);
  background: #fff;
  transition: transform var(--ui-dur-1) var(--ui-ease);
}
.track.on .knob {
  transform: translateX(16px);
}
.native:focus-visible + .track {
  box-shadow: var(--ui-focus);
}
</style>
