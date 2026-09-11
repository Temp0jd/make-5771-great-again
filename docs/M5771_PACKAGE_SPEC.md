# Make 5771 流程分享包 v1

`.m5771pack` 是 Make 5771 Great Again 的单文件流程分享格式。普通用户应在软件的“流程”或“设置”页使用“导出分享包”，然后直接分享生成的文件。

v0.4.8 仅调整视觉主题资源并保留 Windows 中文字体修复；`format_version` 仍为 1，字段、序列化语义和旧分享包兼容规则均未改变。

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
  "app_version": "0.4.8",
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

`profile.idle_scan_secs` 可取 `1`、`3`、`5` 或 `null`。新建流程默认 `3` 秒；视觉步骤上的 `scan_interval_secs` 可覆盖流程值。首次进入步骤会立即扫描，只有未发现候选时才等待该间隔，候选/点击后的稳定确认仍使用约 180 ms。缺少字段的旧包反序列化为 `null`，继续使用 v0.3 的性能档位轮询间隔。

`profile.template_scale_mode` 可取 `UniformFit` 或 `Stretch`。`UniformFit` 按客户区可容纳的统一比例缩放宽高，并将 ROI 映射到居中的内容区，适合跨 16:9、16:10 和超宽屏分享；`Stretch` 沿用旧版宽高分别缩放。新建流程默认 `UniformFit`，缺少该字段的旧包默认 `Stretch`，避免静默改变既有流程。

v0.4.2 起不再提供流程级默认 ROI，以避免模板范围、流程范围和步骤范围三层继承造成歧义。旧包中的 `profile.default_search_region` 会作为未知字段安全忽略。模板自身的 `search_region` 仅供旧版 `Inherit` 搜索策略兼容；新建步骤默认全屏，并推荐直接在每次模板使用处选择全屏、区域优先或严格区域。

`profile.adaptive_roi` 控制旧版继承区域连续未命中后是否允许全屏恢复。新建流程默认启用；缺少该字段的旧包反序列化为 `false`，继续把 ROI 作为严格空间边界，避免兼容升级后发生区域外点击。

每个模板使用位置（步骤 `search`、WaitAny 分支 `search`、分支动作 `search`、视觉条件项 `search`）可独立指定 `strategy`：`Inherit`、`FullFrame`、`FixedRoi`、`RoiThenFullFrame`。流程编辑器可直接截取当前游戏画面并可视化框选步骤级 ROI。`region` 使用同一对象内 `reference_width` × `reference_height` 的坐标空间。`FixedRoi` 是严格安全边界；`RoiThenFullFrame` 才允许恢复全屏。所有字段都带 serde 默认值，旧包缺少 `search` 时等同 `Inherit`。

点击类步骤、WaitAny 分支触发器和分支点击动作可分别携带 `click_count` 与 `click_interval_ms`。次数范围为 1–20，间隔范围为 0–5000 ms；旧包缺少字段时默认单击一次、间隔 100 ms。

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


## v0.4.9：识别后按窗口比例点击

步骤、等待任一分支、分支识别动作以及视觉条件配置支持可选字段：

```json
"relative_click": { "x_percent": 25.0, "y_percent": 75.0 }
```

- 未设置或 `null`：保留原有图片锚点与像素偏移行为，旧流程无需修改。
- 设置后：仍须识别成功并通过原有确认逻辑，随后按整个目标窗口客户区计算点击位置，忽略图片锚点及偏移。坐标不相对于 ROI，也不相对于桌面或多显示器。
- 0% 是左/上边缘，100% 是右/下边缘（限定在最后一个有效像素）；窗口缩放时重新按客户区尺寸计算。比例限制在 0–100。
- 分支关闭触发点击、视觉条件选择不点击结果时，该字段不会触发额外点击。
- 新程序继续接受格式版本 1 的旧分享包；旧程序可能忽略这个新字段而点击原图片位置，因此包含比例落点的包仅应交给支持此功能的新程序执行。
