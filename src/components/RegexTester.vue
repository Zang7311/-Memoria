<!-- 《铃·记忆体》正则表达式测试器（批次4）：输入正则+文本，实时高亮匹配 -->
<script setup lang="ts">
import { ref, computed } from 'vue'

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
  <div class="regex-panel" @click.self="emit('close')">
    <div class="rp-box">
      <div class="rp-head">
        <span class="rp-title">正则表达式测试器</span>
        <button class="btn ghost" @click="emit('close')">关闭</button>
      </div>
      <div class="rp-row">
        <label>正则</label>
        <input v-model="pattern" class="rp-input" placeholder="如：\d+ 或 ^https?://" />
      </div>
      <div class="rp-row">
        <label>标志</label>
        <input v-model="flags" class="rp-input small" placeholder="gi" />
      </div>
      <p v-if="error" class="rp-err">{{ error }}</p>
      <label class="rp-lb">测试文本</label>
      <textarea v-model="text" class="rp-textarea" placeholder="粘贴要测试的文本…" />
      <p class="rp-info">共匹配 {{ matches.length }} 处</p>
      <div class="rp-matches">
        <div v-for="(m, i) in matches" :key="i" class="rp-match">
          <b>{{ m.value }}</b>
          <span class="rp-pos">@{{ m.index }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.regex-panel {
  position: fixed;
  inset: 0;
  z-index: 600;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
}
.rp-box {
  width: 520px;
  max-height: 80vh;
  overflow-y: auto;
  background: var(--bg-bar, rgba(34, 32, 36, 0.95));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  border-radius: 14px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.rp-head { display: flex; align-items: center; justify-content: space-between; }
.rp-title { font-weight: 600; color: var(--text-main, #eee); }
.rp-row { display: flex; align-items: center; gap: 8px; }
.rp-row label { color: var(--text-secondary, #aaa); font-size: var(--fs-13); width: 44px; flex-shrink: 0; }
.rp-input {
  flex: 1;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  background: var(--input-bg, #2a272b);
  color: var(--text-main, #eee);
  font-family: 'Cascadia Mono', Consolas, monospace;
  font-size: var(--fs-13);
}
.rp-input.small { flex: 0 0 70px; }
.rp-lb { color: var(--text-secondary, #aaa); font-size: var(--fs-13); }
.rp-textarea {
  min-height: 160px;
  padding: 10px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  background: var(--input-bg, #2a272b);
  color: var(--text-main, #eee);
  font-family: 'Cascadia Mono', Consolas, monospace;
  font-size: var(--fs-13);
  resize: vertical;
}
.rp-err { color: var(--danger); font-size: var(--fs-12); }
.rp-info { color: var(--text-secondary, #aaa); font-size: var(--fs-12); }
.rp-matches { display: flex; flex-direction: column; gap: 4px; max-height: 140px; overflow-y: auto; }
.rp-match {
  display: flex;
  align-items: center;
  gap: 8px;
  background: rgba(128, 128, 128, 0.12);
  padding: 4px 10px;
  border-radius: 6px;
  font-family: 'Cascadia Mono', Consolas, monospace;
  font-size: var(--fs-13);
  color: var(--text-main, #eee);
  word-break: break-all;
}
.rp-pos { color: var(--accent, #ff7a94); font-size: var(--fs-11); flex-shrink: 0; }
.btn { padding: 6px 14px; border-radius: 8px; border: none; cursor: pointer; font-size: var(--fs-13); background: rgba(128, 128, 128, 0.18); color: var(--text-main, #eee); }
</style>
