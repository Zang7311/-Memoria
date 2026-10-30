// 主题预设的默认色值
//
// 这些不是「样式」，而是**写进用户配置文件的初始数据**（defaults），所以它们必须
// 是具体色值，不能写成 var(--ui-*) —— CSS 变量存进 JSON 后再读出来对任何地方都无效。
//
// 放在单独一处、由 ui-audit 明确豁免，避免「默认色散落在业务代码里」演变成新的漂移源。
export const THEME_DEFAULT = {
  /** 主色（粉） */
  accent: '#ff7a94',
  /** 深色背景 */
  background: '#1d1b1f',
  /** 主人气泡底色 */
  bubbleUser: '#2d2d2d',
  /** 铃气泡底色 */
  bubbleSuzu: '#3a3438',
} as const
