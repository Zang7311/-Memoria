// 《铃·记忆体》内置示例插件：文件检索（基础版）
// 展示标准插件写法：
// 1. 入口文件定义 globalThis.skills = { 技能名: function(params){...} }
// 2. 高危操作一律通过 invoke_plugin 白名单命令（沙箱校验权限）
// 3. 权限声明见 manifest.json（permissions: ["file.read"]）
//
// invoke_plugin 说明：返回 JSON 字符串，这里用 JSON.parse 解析。
globalThis.skills = {
  file_search: function (params) {
    var keyword = params.keyword || '';
    if (!keyword) {
      return { success: false, error: '缺少 keyword 参数' };
    }
    var dir = params.dir || '';
    console.log('开始搜索：', keyword, dir ? '（目录：' + dir + '）' : '（默认目录）');

    // 调用沙箱白名单命令（需要 file.read 权限，未授权会被拒绝）
    var raw = invoke_plugin('file.search', { keyword: keyword, dir: dir });
    var result = JSON.parse(raw);
    if (result.count === 0) {
      return { success: true, message: '没有找到包含「' + keyword + '」的文件' };
    }
    return {
      success: true,
      message: '找到 ' + result.count + ' 个文件：',
      files: result.files.slice(0, 10)
    };
  }
};
