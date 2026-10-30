<!-- 《铃·记忆体》正则表达式测试器（工具箱弹窗）：输入正则 + 测试文本，实时列出匹配 -->
<script setup lang="ts">
import { ref, computed } from 'vue'
import TheIcon from './TheIcon.vue'
import AppStatusDot from './ui/AppStatusDot.vue'

const emit = defineEmits<{ close: [] }>()
const pattern = ref('')
const flags = ref('gi')
const text = ref('')

const error = computed(() => {
  try {
    new RegExp(pattern.value, flags.value)
    return ''
  } catch (e) {
    return String(e)
  }
})

const matches = computed(() => {
  if (!pattern.value || error.value) return []
  try {
    const re = new RegExp(pattern.value, flags.value)
    const arr: { index: number; value: string }[] = []
    let m: RegExpExecArray | null
    while ((m = re.exec(text.value)) !== null) {
      arr.push({ index: m.index, value: m[0] })
      if (m.index === re.lastIndex) re.lastIndex++
    }
    return arr
  } catch {
    return []
  }
})
</script>

<template>
  <div class="regex-mask" @click.self="emit('close')">
    <div class="regex-box" role="dialog" aria-modal="true" aria-label="正则表达式测试器">
      <header class="rx-head">
        <span class="rx-title">正则表达式测试器</span>
        <button class="rx-close" type="button" title="关闭" aria-label="关闭" @click="emit('close')">
          <TheIcon name="close" :size="15" />
        </button>
      </header>

      <div class="rx-row">
        <label class="rx-label" for="rx-pattern">正则</label>
        <input id="rx-pattern" v-model="pattern" class="rx-input" placeholder="如：\d+ 或 ^https?://" />
      </div>
      <div class="rx-row">
        <label class="rx-label" for="rx-flags">标志</label>
        <input id="rx-flags" v-model="flags" class="rx-input small" placeholder="gi" />
      </div>

      <p v-if="error" class="rx-err" role="alert">
        <AppStatusDot state="err" text="正则错误" />
        <span class="rx-err-text">{{ error }}</span>
      </p>

      <label class="rx-label" for="rx-text">测试文本</label>
      <textarea id="rx-text" v-model="text" class="rx-textarea" placeholder="粘贴要测试的文本…" />

      <p class="rx-info">共匹配 {{ matches.length }} 处</p>

      <div v-if="matches.length" class="rx-matches">
        <div v-for="(m, i) in matches" :key="i" class="rx-match">
          <b class="rx-hit">{{ m.value }}</b>
          <span class="rx-pos">@{{ m.index }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 弹窗遮罩（工具箱内嵌弹窗，本身就是覆盖层） */
.regex-mask {
  position: fixed;
  inset: 0;
  z-index: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--ui-sp-4);
  background: color-mix(in srgb, var(--ui-bg) 78%, transparent);
}

.regex-box {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
  width: min(520px, 100%);
  max-height: 86vh;
  overflow-y: auto;
  padding: var(--ui-sp-4);
  background: var(--ui-raised);
  color: var(--ui-text);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-lg);
  box-shadow: var(--ui-shadow);
  font-size: var(--ui-fs-13);
}

.rx-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.rx-title {
  min-width: 0;
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
  overflow-wrap: anywhere;
}
.rx-close {
  display: grid;
  place-items: center;
  flex: none;
  width: var(--ui-sp-6);
  height: var(--ui-sp-6);
  padding: 0;
  background: transparent;
  color: var(--ui-text-2);
  border: 0;
  border-radius: var(--ui-r-sm);
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.rx-close:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}

.rx-row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.rx-label {
  flex: none;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
}
.rx-input {
  flex: 1 1 auto;
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-2);
  background: var(--ui-bg);
  color: var(--ui-text);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  font-family: var(--font-mono);
  font-size: var(--ui-fs-13);
}
.rx-input:focus {
  border-color: var(--ui-accent);
}
.rx-input::placeholder {
  color: var(--ui-text-3);
}
.rx-input.small {
  flex: 0 1 88px;
}

.rx-err {
  display: flex;
  align-items: flex-start;
  gap: var(--ui-sp-2);
  margin: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  background: color-mix(in srgb, var(--ui-err) 10%, transparent);
  border-left: 3px solid var(--ui-err);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
}
.rx-err-text {
  min-width: 0;
  color: var(--ui-err);
  overflow-wrap: anywhere;
}

.rx-textarea {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: 160px;
  padding: var(--ui-sp-2);
  background: var(--ui-bg);
  color: var(--ui-text);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  font-family: var(--font-mono);
  font-size: var(--ui-fs-13);
  line-height: var(--ui-line);
  resize: vertical;
}
.rx-textarea:focus {
  border-color: var(--ui-accent);
}
.rx-textarea::placeholder {
  color: var(--ui-text-3);
}

.rx-info {
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
}

.rx-matches {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  max-height: 180px;
  overflow-y: auto;
}
.rx-match {
  display: flex;
  align-items: baseline;
  gap: var(--ui-sp-2);
  min-width: 0;
  padding: var(--ui-sp-1) var(--ui-sp-2);
  background: var(--ui-bg);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  font-family: var(--font-mono);
  font-size: var(--ui-fs-13);
  color: var(--ui-text);
}
/* 命中内容高亮：主色浅底（语义色，不写死） */
.rx-hit {
  min-width: 0;
  padding: 0 var(--ui-sp-1);
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
  border-radius: var(--ui-r-sm);
  font-weight: 600;
  overflow-wrap: anywhere;
}
.rx-pos {
  flex: none;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
</style>
