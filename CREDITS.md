# 美术素材来源（CREDITS）

本项目为**非官方同人工具**，与《忘却前夜》（Morimens）的开发商、发行商及运营商无任何关联。
下列素材的著作权归原权利人（B.I.A.V. Studio / Qookka Games 等）所有，仅用于**非商业的个人学习与交流**；
权利人如有异议，请通过 [GitHub Issues](https://github.com/Temp0jd/make-5771-great-again/issues) 联系，我们会立即移除相关文件。

素材**不适用**本项目的 MIT 许可证，其再使用须遵循原权利人的规定。

## 内嵌背景立绘（`assets/art/portraits/`）

用于运行时随机显示的半透明背景装饰。文件经裁切与降采样（最长边 ≤512px）后内嵌进程序。

| 文件 | 原始文件 | 来源 |
| --- | --- | --- |
| `agrippa-portrait-card.png` | Agrippa_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/e/ec/Agrippa_Portrait_Card.png/revision/latest?cb=20260722003926) |
| `aigis-portrait-card.png` | Aigis_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/8/83/Aigis_Portrait_Card.png/revision/latest?cb=20260722003559) |
| `alva-portrait-card.png` | Alva_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/e/eb/Alva_Portrait_Card.png/revision/latest?cb=20260722002245) |
| `aurita-portrait-card.png` | Aurita_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/8/8f/Aurita_Portrait_Card.png/revision/latest?cb=20260722011831) |
| `casiah-portrait-card.png` | Casiah_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/0/08/Casiah_Portrait_Card.png/revision/latest?cb=20260721234910) |
| `castor-portrait-card.png` | Castor_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/c/c8/Castor_Portrait_Card.png/revision/latest?cb=20260721234958) |
| `celeste-portrait-card.png` | Celeste_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/c/c7/Celeste_Portrait_Card.png/revision/latest?cb=20260722012053) |
| `clementine-portrait-card.png` | Clementine_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/9/9e/Clementine_Portrait_Card.png/revision/latest?cb=20260721235606) |
| `daffodil-portrait-card.png` | Daffodil_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/9/97/Daffodil_Portrait_Card.png/revision/latest?cb=20260721235309) |
| `ogier-portrait-card.png` | Ogier_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/1/1e/Ogier_Portrait_Card.png/revision/latest?cb=20260722001309) |
| `ramona-timeworn-portrait-card.png` | Ramona-Timeworn_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/c/cb/Ramona-Timeworn_Portrait_Card.png/revision/latest?cb=20260722000957) |
| `wanda-portrait-card.png` | Wanda_Portrait_Card.png (406x614) | [Morimens Fandom Wiki](https://static.wikia.nocookie.net/forget-last-night-morimens/images/2/2c/Wanda_Portrait_Card.png/revision/latest?cb=20260721234440) |

## 内嵌表情包（`assets/emoji/`）

界面中用于状态反馈与装饰的 17 张表情，来源与上面相同（Morimens 社区 wiki 镜像），沿用项目原有使用范围。

## 候选素材（`art-candidates/`，未内嵌）

`art-candidates/` 是按同样方式收集的**候选**素材（头像 / 表情 / 立绘 / 纹章），**不参与编译**，也不会进入分享包。
每张图的来源链接见 `art-candidates/manifest.json`；确定使用后会迁入 `assets/art/` 并补充到本文件。

> 收集方式：Fandom wiki 的 MediaWiki API 枚举公开文件并下载原始文件，经 Pillow 裁切/降采样。没有解包游戏客户端，也没有绕过登录或付费内容。
