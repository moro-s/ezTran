# EzTran

轻量级桌面翻译工具，基于 Rust + egui 构建。

## 功能

- **输入翻译**：左右分栏布局，左侧输入原文，右侧显示译文
- **划词翻译**：从剪贴板读取选中文本进行翻译
- **多引擎支持**：有道翻译、百度翻译、DeepL、自定义 API
- **系统托盘**：托盘菜单快速切换翻译/设置页面，关闭窗口时最小化到托盘
- **配置持久化**：引擎配置和语言偏好保存在本地 JSON 文件

## 技术栈

| 组件 | 说明 |
|------|------|
| GUI 框架 | [egui](https://github.com/emilk/egui) / eframe |
| HTTP 客户端 | [minreq](https://github.com/neonmoe/minreq) (rustls) |
| 剪贴板 | [arboard](https://github.com/1Password/arboard) |
| 系统托盘 | [tray-icon](https://github.com/tauri-apps/tray-icon) |
| 序列化 | serde / serde_json |

## 构建与运行

```bash
# 开发模式
cargo run

# 发布构建
cargo build --release
```

## 项目结构

```
src/
├── main.rs             # 程序入口、系统托盘、字体加载
├── config.rs           # 配置加载/保存、语言列表
├── ui.rs               # egui UI（翻译页 + 设置页）
└── translate/
    ├── mod.rs          # 模块导出
    ├── engine.rs       # 引擎配置定义
    └── translator.rs   # 翻译执行逻辑
```

## 配置

配置文件路径：`~/.config/eztran/config.json`（Windows 为 `%APPDATA%\eztran\config.json`）

在设置页面中配置翻译引擎的 API Key 和 Secret 即可使用。

## License

MIT
