<div align="center">

<img src="assets/icon.png" width="140" alt="誓死效忠忘却前夜">

### 校猫罪大恶极，搞得玩家怨声载道

# Make 5771 Great Again

**面向 Windows 11 的《忘却前夜》轻量视觉流程编辑器**

截图识别 × 自动点击 × 键盘注入 × 流程分享——只读屏幕画面，不读不写游戏进程

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

## 下载

从 [Releases](https://github.com/Temp0jd/make-5771-great-again/releases) 下载最新的 `Make5771GreatAgain.exe`，免安装、单文件运行。程序数据（流程、模板、日志）保存在 exe 所在目录下。

每个 Release 还附带 **`template.zip`**：打包了全部官方示例分享包（见下文「分享流程」）。下载后解压，得到若干 `.m5771pack` 文件，在「流程」页点击「导入分享包」逐个导入，或直接拖入程序窗口。

## 功能一览

- **流程编排**：等待并点击、等待任一目标、视觉条件（AND/OR + 稳定确认）、键盘输入、固定等待、本局结束；分支子动作与优先级；固定次数、截止时间、持续运行三种循环策略；多流程一键切换
- **精准点击**：模板图上可视化选点（锚点 + 像素偏移），前台/后台两种点击方式，拟人化抖动
- **键盘注入**：中文文本键入（速率可调、节奏拟人）、组合键（如 `ctrl+c`、`alt+f4`），实时语法校验
- **执行安全**：失去前台自动暂停、两段式停止（礼貌停止 / 确认后强制停止）、客户区尺寸保护、窗口掉线自动重连、截止时间跨午夜顺延
- **模板工具**：全局热键快速截图框选（热键可自定义）、PNG/JPG 拖放导入、局部搜索区域、后台线程识别测试、匹配位置可视化、分辨率不符自动缩放
- **识别引擎**：彩色 RGB 匹配 + 有效像素加权（模板背景不参与打分，抗误触）+ 积分图下界过滤与多线程，1080p 全图约 3-10 ms；点击前二次确认防闪烁误触
- **分享协作**：`.m5771pack` 单文件导出（内嵌模板图）、拖放导入、多流程文件管理
- **可观测性**：每局耗时与预计剩余时间、超时输出最佳相似度、日志落盘、崩溃 crash.log、系统托盘与完成通知

## 快速上手

1. **连接窗口**：运行页点击「连接窗口」，选中游戏窗口。
2. **截取模板**：按截图热键（默认 `F6`）框选「开始」「Auto」「结算」等目标画面。
3. **编排流程**：流程页添加步骤——识别到某图就点击、敲键盘或等待；点击位置可在模板图上直接点选。
4. **开始运行**：设置循环方式后点击「开始运行」；按停止热键（默认 `F8`）或界面按钮随时停止。

## 执行模型

流程为线性步骤序列，配合外层循环执行。步骤类型：

| 步骤 | 说明 |
| --- | --- |
| 等待并点击 | 识别到目标图片后点击（可自定义点击位置），超时失败 |
| 等待任一目标 | 多分支按优先级匹配，命中后执行子动作，再决定返回等待 / 继续 / 完成本局 / 停止 |
| 视觉条件 | AND/OR 组合多条「出现 / 不出现」检查，稳定确认后触发动作 |
| 键盘输入 | 文本键入（中文可、速率可调）或组合键，执行前自动激活窗口，仅前台有效 |
| 固定等待 | 原地等待指定时长 |
| 本局结束 | 计入完成局数并开始下一轮 |

执行层只使用屏幕画面和 Windows 标准输入接口（`SendInput`），不读取或修改目标进程。

## 快捷键

| 功能 | 默认热键 | 说明 |
| --- | --- | --- |
| 截取游戏画面 | `F6` | 最小化程序并框选截图 |
| 停止运行 | `F8` | 请求停止当前流程 |

两个热键均可在「设置」页修改（支持 `ctrl` / `shift` / `alt` 组合），按流程保存、随流程切换自动生效。

## 分享流程

1. 在「设置 > 分享信息」填写作者、游戏版本、语言和说明。
2. 在「流程」页点击「导出分享包」。
3. 分享生成的 `.m5771pack` 单文件，不需要另外附带 `templates` 目录。
4. 接收者点击「导入分享包」，或将文件直接拖入程序。

格式详细说明见 [docs/M5771_PACKAGE_SPEC.md](docs/M5771_PACKAGE_SPEC.md)。

官方示例包（Release 的 `template.zip` 内含全部三个，也可在 [examples/](examples/) 单独获取）：

- [Auto 起步包](examples/auto-starter.m5771pack)：不含图片的起步骨架，导入后自行绑定截图；
- [活动关卡（刷同调）示例包](examples/morimens-event-tongdiao.m5771pack)：主循环常驻监视——通关自动开新局并计数、死亡自动重开、活动Token随选随确认、Auto 未开启才点，含 11 张模板；
- [Token 选择范例包](examples/token-select-example.m5771pack)：演示「出现 A → 选择 A → 确认」的通用模式。

## 本地开发

```powershell
cargo run --release
```

Windows 发布构建：

```powershell
cargo build --release
```

输出文件位于 `target\release\Make5771GreatAgain.exe`。

提交前请确保通过：

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets
```

推送 `v*` 标签会自动触发 CI（fmt / clippy / test / 发布构建）并发布 GitHub Release。

## 美术素材与版权声明

本软件为**非官方同人工具**，与《忘却前夜》（Morimens）的开发商、发行商及运营商无任何关联，亦未获得其授权或认可。

项目中使用的应用图标、表情包等装饰性美术素材，其原型来源于《忘却前夜》游戏及相关同人创作，**著作权归原权利人（B.I.A.V. Studio / Qookka Games 等）所有**。这些素材仅以非商业、个人学习交流为目的在本项目中使用，不用于任何商业用途；若涉及侵权或权利人有异议，请通过 [GitHub Issues](https://github.com/Temp0jd/make-5771-great-again/issues) 联系，本人将立即移除相关内容。

上述美术素材**不适用**于本项目的 MIT 许可证，其再使用须遵循原权利人的相关规定。

## 许可证

本项目的**源代码**以 [MIT](LICENSE) 许可证发布（美术素材除外，见上节）。
