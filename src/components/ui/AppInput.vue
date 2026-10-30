<!-- 统一输入框：底 raised，边 strong，聚焦只换边色不发光 -->
<script setup lang="ts">
withDefaults(
  defineProps<{
    modelValue: string
    placeholder?: string
    type?: 'text' | 'password' | 'number'
    disabled?: boolean
  }>(),
  { placeholder: '', type: 'text', disabled: false },
)
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLInputElement).value)
}
</script>

<template>
  <input
    class="app-input"
    :type="type"
    :value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    @input="onInput"
  />
</template>

<style scoped>
.app-input {
  width: 100%;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
  background: var(--ui-raised);
  color: var(--ui-text);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-13);
  transition: border-color var(--ui-dur-1) var(--ui-ease);
}
.app-input::placeholder {
  color: var(--ui-text-3);
}
.app-input:focus {
  border-color: var(--ui-accent);
}
</style>
