# Make 5771 流程分享包 v1

`.m5771pack` 是 Make 5771 Great Again 的单文件流程分享格式。普通用户应在软件的“流程”或“设置”页使用“导出分享包”，然后直接分享生成的文件。

## 包含内容

分享包是 UTF-8 JSON，包含：

- 格式标识和版本号；
- 创建应用版本和时间；
- 完整的流程配置；
- 所有被流程使用的 PNG 模板，以标准 Base64 嵌入。

顶层结构：

```json
{
  "format": "make5771.workflow-package",
  "format_version": 1,
  "app_version": "0.3.11",
  "created_at": "RFC 3339 timestamp",
  "profile": {},
  "assets": [
    {
      "key": "assets/template-1.png",
      "byte_length": 1234,
      "data_base64": "..."
    }
  ]
}
```

`profile.templates[*].path` 和步骤中的模板引用都使用 `assets[*].key`，不应包含作者电脑上的绝对路径。

`profile.recognition_performance` 可取 `Eco`、`Balanced` 或 `Responsive`，分别限制为 2、4 或最多 8 个识别线程。缺少该字段的旧包默认使用 `Balanced`，识别判定规则不变。

`profile.adaptive_roi` 控制配置区域连续未命中后是否允许全屏恢复。新建流程默认启用；缺少该字段的旧包反序列化为 `false`，继续把 ROI 作为严格空间边界，避免兼容升级后发生区域外点击。

`profile.sharing` 用于社区展示和兼容性判断：

```json
{
  "author": "作者名",
  "description": "流程用途、入口画面与已知限制",
  "game_version": "已测试的游戏版本",
  "game_language": "简体中文",
  "tags": "Auto,日常,刷关"
}
```

## 导入安全限制

- 只接受 `format_version: 1`；
- 整包不超过 128 MiB；
- 最多 500 张模板；
- 单张 PNG 不超过 16 MiB；
- 图片宽高不超过 8192 像素；
- 资源键不会被当作本地解压路径；
- 不支持脚本、DLL、EXE 或其他可执行附件。

导入后，软件会为该包创建独立的 `imports/package-<timestamp>/templates/` 目录，并将流程内部引用重写为本机路径。

## 社区分享建议

发布包时建议同时说明：

- 适用的游戏语言和 UI 缩放；
- 目标客户区尺寸；
- 流程用途和预期入口画面；
- 需要用户重新截图的模板；
- 测试过的游戏版本和日期。

画面模板会因分辨率、语言、游戏更新或显示效果而失效。导入他人的包后，应先在“模板”页逐一测试，再使用少量循环试运行。相似按钮应只截取具有区分度的文字或图标。启用智能 ROI 越界恢复后，局部搜索区域是性能提示；关闭时则是严格边界。透明 PNG 的 Alpha=0 像素不会参与识别。
