// 《铃·记忆体》内置插件：文件查看器
// 演示点：JS 技能通过沙箱白名单 invoke_plugin('file.read_text') 读取文件，
// 需要 manifest 声明 file.read 权限并被用户授予（内置插件默认预授权）。
globalThis.skills = {
  read_file: function (params) {
    var path = params.path || '';
    if (!path) {
      return { success: false, error: '缺少 path 参数' };
    }
    console.log('读取文件：', path);

    // 沙箱白名单调用（未授予 file.read 权限会被拒绝并抛错）
    var raw = invoke_plugin('file.read_text', { path: path });
    var result = JSON.parse(raw);
    var content = result.content || '';
    return {
      success: true,
      path: result.path,
      length: content.length,
      preview: content.slice(0, 200)
    };
  }
};
