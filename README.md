# EzTran

轻量级桌面翻译工具，基于 Rust + egui 构建。

## 功能

- **输入翻译**：左右分栏布局，左侧输入原文，右侧显示译文，支持 `Ctrl+Enter` 快捷翻译
- **划词翻译**：全局热键读取剪贴板选中文本，自动弹出窗口翻译
- **多引擎支持**：有道翻译、百度翻译、DeepL、自定义 API（兼容 OpenAI 格式）
- **翻译历史**：最近 50 条翻译记录，独立窗口查看，持久化到磁盘
- **系统托盘**：托盘菜单快速唤起翻译/设置，关闭窗口时最小化到托盘
- **全局热键**：可自定义划词翻译与输入翻译快捷键
- **窗口置顶**：钉住按钮一键置顶，始终浮于其他窗口之上
- **主题切换**：深色 / 浅色 / 跟随系统
- **字号调节**：小 / 标准 / 大三档字号
- **配置持久化**：引擎配置、语言偏好、主题等保存在本地 JSON 文件

## 技术栈

| 组件 | 说明 |
|------|------|
| GUI 框架 | [egui](https://github.com/emilk/egui) 0.36 / eframe |
| HTTP 客户端 | [minreq](https://github.com/neonmoe/minreq) (rustls) |
| 剪贴板 | [arboard](https://github.com/1Password/arboard) |
| 系统托盘 | [tray-icon](https://github.com/tauri-apps/tray-icon) |
| 序列化 | serde / serde_json |
| 日志 | simplelog + chrono（按天存储，自动清理 7 天前日志） |

## 构建与运行

```bash
# 开发模式
cargo run

# 发布构建
cargo build --release
```

> 仅支持 Windows 平台（窗口管理、全局热键、字体加载使用 Win32 API）。

## 项目结构

```
src/
├── main.rs             # 程序入口、日志初始化、系统托盘
├── app.rs              # eframe App 实现、自绘标题栏、托盘菜单
├── config.rs           # 配置加载/保存、主题/字号枚举、语言列表
├── history.rs          # 翻译历史持久化（最近 50 条）
├── hotkey.rs           # 全局热键注册与监听线程
├── icon.rs             # 窗口/托盘图标生成
├── theme.rs            # 字体加载、主题配色、样式设置
├── window.rs           # Win32 窗口管理（显示/隐藏/置顶/圆角）
├── translate/
│   ├── mod.rs          # 模块导出
│   ├── engine.rs       # 引擎类型定义与配置
│   └── translator.rs   # 各引擎翻译执行逻辑
└── ui/
    ├── mod.rs          # 模块导出
    ├── components.rs   # 公共 UI 组件（动画、Toast）
    ├── state.rs        # 全局应用状态
    ├── translate.rs    # 翻译工作台 UI
    ├── settings.rs     # 设置页面 UI
    └── history.rs      # 历史记录窗口 UI
```

## 配置

配置文件路径：`%APPDATA%\eztran\config.json`

在设置页面中配置翻译引擎的 API Key 和 Secret 即可使用。支持配置项：

- 默认源/目标语言
- 划词翻译 / 输入翻译快捷键
- 应用主题与字号
- 引擎开关与自启动

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+T` | 划词翻译（默认） |
| `Ctrl+Shift+I` | 输入翻译（默认） |
| `Ctrl+Enter` | 翻译工作台内触发翻译 |
| `Esc` | 隐藏翻译工作台到托盘 |

快捷键可在设置页面中自定义。

## License

MIT
