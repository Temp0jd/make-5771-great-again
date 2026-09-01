<div align="center">

<img src="assets/icon.png" width="140" alt="誓死效忠忘却前夜">

# 校猫罪大恶极，搞得玩家怨声载道

### Make 5771 Great Again

面向 Windows 11 的《忘却前夜》轻量视觉流程编辑器
截图识别 × 自动点击 × 流程分享——只读屏幕画面，不读不写游戏进程

[![Release](https://img.shields.io/github/v/release/Temp0jd/make-5771-great-again)](https://github.com/Temp0jd/make-5771-great-again/releases)
[![CI](https://github.com/Temp0jd/make-5771-great-again/actions/workflows/ci.yml/badge.svg)](https://github.com/Temp0jd/make-5771-great-again/actions)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)](https://www.rust-lang.org/)
[![egui](https://img.shields.io/badge/egui-0.35-blue)](https://github.com/emilk/egui)
[![Platform](https://img.shields.io/badge/platform-Windows%2011-0078D6?logo=windows)](https://www.microsoft.com/windows)

</div>

---

### 《弥萨格猫公德政碑》

<img src="assets/emoji/kekesi-cry.png" width="72" alt="凯刻斯·大哭">

弥萨格里有猫公，一掌乾坤万策通。
玩家但求三分暖，猫公偏爱试严冬。

昔闻新角将临世，不问人物先问盅。
杯虽未售名先震，二游千载仰遗风。
世人笑问谁开祖？遥指校园一猫公。

其后民声喧似海，猫公垂耳坐帘中。
白昼无言称未见，夜来旧律换新容。
问则不知，查则无事，
待君再看——已与昨不同。

<img src="assets/emoji/wincor-run.png" width="72" alt="温柯尔·逃跑：我听不见我看不见">

曾有复活惊天地，残灯将灭又重红。
众生奔走呼赢矣，猫公归来第一功。
自此江湖传妙语：
游戏尚能打复活，
玩家唯恐猫复生。

昔年又立“歉意”榜，字字诚恳泪朦胧。
五十银芯平天下，一封长信谢诸公。
昨日亲书“吾有过”，
明朝妙策胜前踪。
旧疮方结新疮起，
可见猫公善始终。

更有血希沉旧狱，玩家早已释前嫌；
策划胸怀真似海，
至今犹未肯原谅。

融灾场上水初温，众人尚道可从容。
猫公闻罢摇头笑：
“既称容灾，岂容温？”
遂撤薪柴添猛火，
铁锅一夜赤如铜。
世人只求留退路，
猫公提壶泼沸洪。

战至力竭何须惧？
山中自有九芝供。
一株入口魂还魄，
再株入口见猫公。
若问何故常含笑，
灵芝嚼罢气色红。

<img src="assets/emoji/dexter-cheers.png" width="72" alt="德克斯特·干杯">

今朝又铸同心证，
金字煌煌曰“同心”。
一证方开一重锁，
五关层叠五重门。
四市月颁寥寥纸，
万千旧友候晨昏。

二万一千方起步，
三万一千五百终；
十二万六千情与义，
方换曜闪一点红。

尤恐诸君情不笃，
手操特赐三倍功。
自动岂知相思苦？
亲手坐牢始情浓。
厨力既将廿四满，
莫言从此得从容：
密契刷毕无所事？
请来为猫作牛工。

<img src="assets/emoji/wanda-work.png" width="72" alt="旺达·一定能做完">

于是玩家皆顿悟：
所谓同调不在“同”，
所谓同心不在“心”。
心若真同何须证？
证若难求最情深。

猫公治校真奇绝，
聋可为听，改可称功；
慢可谓长久陪伴，
烫可谓水温包容；
牢可谓内容丰富，
肝可谓情谊深浓。

呜呼——

杯未曾出梗长在，
芝尚未尽水先沸；
歉书叠作弥萨山，
同心证锁守密人。

若问前夜何以忘？
玩家未忘猫先忘。
若问此游何处弱？
美术非也，音乐非也，
关卡亦未必也——

史官掷笔长叹曰：

天下策病千千万，
唯此一猫最难防。

<img src="assets/emoji/ramona-point.png" width="72" alt="拉蒙娜·指：给我玩忘却前夜！">

---

## 功能一览

- **界面**：iOS 风格中文界面、自绘无边框窗口、运行/流程/模板/日志/设置五页、忘却前夜表情装饰
- **流程编排**：等待并点击、等待任一目标、视觉条件（AND/OR + 稳定确认）、固定等待、本局结束；分支子动作与优先级；固定次数、截止时间、持续运行三种循环策略
- **执行安全**：失去前台自动暂停、F8 紧急停止、客户区尺寸保护、窗口掉线自动重连、截止时间跨午夜顺延、前台/后台两种点击方式与拟人化抖动
- **模板工具**：F6 快速截图框选、PNG/JPG 拖放导入、局部搜索区域建议、识别测试与匹配位置可视化、模板缩略图预览、分辨率不符自动缩放
- **分享协作**：`.m5771pack` 单文件导出（内嵌模板图）、拖放导入、多流程文件管理
- **可观测性**：每局耗时与预计剩余时间、超时输出最佳相似度、日志落盘、崩溃 crash.log、系统托盘与完成通知

## 执行模型

执行器支持三类线性步骤（等待并点击 / 固定等待 / 本局结束）、外层循环、可执行的“等待任一目标”分支，以及“视觉条件”步骤：按 AND/OR 组合多条出现/不出现检查，连续稳定确认后可继续流程、点击命中模板、完成本局或停止任务。画面分支按列表优先级匹配，命中后可点击触发目标、执行线性子动作，再返回等待、继续后续步骤、完成本局或停止任务。通用 If/Else 条件节点仍在后续计划中。

执行层只使用屏幕画面和 Windows 标准输入接口，不读取或修改目标进程。

## 分享流程

1. 在“设置 > 分享信息”填写作者、游戏版本、语言和说明。
2. 在“流程”页点击“导出分享包”。
3. 分享生成的 `.m5771pack` 单文件，不需要另外附带 `templates` 目录。
4. 接收者点击“导入分享包”，或将文件直接拖入程序。

格式详细说明见 [docs/M5771_PACKAGE_SPEC.md](docs/M5771_PACKAGE_SPEC.md)。项目还提供了不含图片的 [Auto 起步包](examples/auto-starter.m5771pack)（导入后可自行绑定截图），以及[活动刷同调示例包](examples/morimens-event-tongdiao.m5771pack)（含“完成调查/取消”等 11 张模板）。

## 本地开发

```powershell
cargo run --release
```

Windows 发布构建：

```powershell
cargo build --release
```

输出文件位于 `target\release\Make5771GreatAgain.exe`。推送 `v*` 标签会自动触发 CI 构建并发布 GitHub Release。
