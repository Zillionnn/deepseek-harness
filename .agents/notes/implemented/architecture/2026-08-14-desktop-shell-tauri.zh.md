# Agent Note: 桌面壳 surface(Tauri)

Status: implemented

English | [中文](2026-08-14-desktop-shell-tauri.zh.md)

## Problem

harness 唯一的交互式 GUI 是浏览器 surface:启动它需要在终端里敲 `pnpm dsh --profile web`,进程存活期间会一直挂着一个控制台窗口,UI 则活在浏览器标签页里。没有双击入口,也没有原生窗口。本 fork 想要类似 Codex Desktop 的体验:双击一个可执行文件,得到桌面窗口,没有控制台。

## Decision

### 壳是原生 Tauri v2 应用,不是 Cordis 插件

`packages/desktop/app`(`@deepseek-ai/dsh-desktop`)是一个 Tauri v2 应用,核心是 Rust:它把 harness 后端作为隐藏子进程拉起来,并在内嵌 WebView2 窗口(Windows 系统运行时,不捆绑 Chromium)中渲染 UI。壳是既有 wire 上的一个新载波——它加载的 web surface 是未改动的 `dsh-web-app` bundle——因此刻意不是 Cordis 插件,不拥有任何 harness 行为。没有独立的 TypeScript 插件包;包内有一个空的 TypeScript 面(`src/index.ts`)及 tsconfig/tsdown 配置,只为让 workspace 构建 glob(`packages/*/*`)与 host 聚合接受它;spawn 规格与握手解析都在 Rust 里,由 cargo 单元测试覆盖。

### 后端子进程与 stdout 握手

壳以 `current_dir` = 仓库根、Windows 上带 `CREATE_NO_WINDOW`、stdin 置 null、stdout/stderr 管道化的方式 spawn `node <repo>/apps/cli/lib/bin.js --profile web --port 0 --no-open`。`--port 0` 让 OS 选空闲端口;`--no-open` 阻止后端打开默认浏览器(壳自己用 WebView2 托管 URL)。web-app bundle 默认 `printUrl: true`,所以后端在其 Loader 树安定后会在 stdout 打印 `dsh web: http://127.0.0.1:<port>`(可能带 ` (LAN: ...)` 后缀)。壳读 stdout 行直到 URL 行出现,把该 URL 载入主窗口,然后继续把 stdout 读到 EOF 以免管道阻塞。stderr 被排进一个有上限的尾部缓冲,后端死亡时错误对话框会带上它。

壳与后端不得共享控制台:后端用 `CREATE_NO_WINDOW` 拉起,壳的 release 构建带 `windows_subsystem = "windows"`。

### 路径解析

`DSH_DESKTOP_BACKEND_CWD`(运行时环境变量,完全覆盖)优先;否则用编译期烤入的仓库根(`CARGO_MANIFEST_DIR` 上溯四级)作为仓库根;否则启动即大声失败,弹对话框指明变量名。`DSH_DESKTOP_NODE` 覆盖 node 可执行文件(默认从 `PATH` 解析 `node`)。CLI 路径是 `<cwd>/apps/cli/lib/bin.js`,因此后端要求先跑过 `pnpm run build`(web-app bundle 在前端 dist 缺失时会在激活期大声失败)。

### 生命周期:单实例,关窗即退

`tauri-plugin-single-instance` 在第二次启动时聚焦已有窗口。关窗即退出:壳把子进程标记为用户终止,杀掉进程树(Windows 上是 `taskkill /PID <pid> /T /F`,其他平台 `child.kill()`)。会话数据按事件追加持久化,强杀不会丢已提交数据。后端在没有用户关闭的情况下退出时,壳弹出带 stderr 尾部的对话框并退出。没有托盘、没有自动重启、没有安装器:`bundle.active` 为 `false`,`tauri build` 只产出便携 exe。v1 仅面向 Windows,但代码为其他平台保留了 `cfg` 分支。

### 验证

按 fork 的里程碑范围,只有 cargo 单元测试:spawn 规格解析顺序(环境变量覆盖、烤入回退、大声失败)、URL 行解析(前缀、空白、`http://` 要求、LAN 后缀截断)、后端退出分类(用户发起 vs 意外)。v1 不做集成冒烟,也没有手动清单。

## Alternatives considered

**Electron。** 参照产品(Codex Desktop)就是 Electron,第一版方案也选了它。因重量被否:捆绑 Chromium 意味着约 200MB 下载和一个吃内存的运行时,而壳的职责只是托管一个 loopback URL。Tauri 复用系统 WebView2,还让 OS webview 无需应用更新即可保持最新。

**裸 WebView2 宿主。** 手写的最小宿主(Rust/C#/C++)比 Tauri 小几 MB,但窗口管理、单实例、错误对话框、打包与未来的跨平台支持全都要手搓。Tauri 用一个依赖换来这些,符合仓库的「优先维护良好的依赖」原则。

**隐藏控制台启动器 + 默认浏览器。** 一个静默拉起后端并打开默认浏览器的微型 GUI 桩能零成本解决控制台痛点,但没有桌面窗口,与明确的「像 Codex Desktop」目标相悖,被否。

**后端跑在壳进程内。** 把 harness 放进壳进程会耦合生命周期,并撞上 Node 版本下限(`^22.19 || >=24`):Electron 等内嵌运行时带的 Node 更旧。子进程原样复用既有 host↔client wire,让壳与 harness 各自独立。

## Consequences

**买到**:Windows 上双击即得原生窗口、无控制台;整个 web surface、wire 与后端组合原样工作(loopback 载波正是浏览器信任围栏与原生操作门禁已经支持的形态);单实例与大声错误对话框;无安装器的小巧便携 exe;后续分发可以直接把 Node 绑进既有 pkg `--sea` 管线,无需改动壳。

**付出**:fork 在基本全 TypeScript/C 的仓库里多了一条 Rust 工具链(rustup + MSVC);WebView2 是系统运行时依赖(Win10/11 预装);spawn 规格与握手逻辑在壳与未来其他载波之间是复制而非共享;v1 仅 Windows,且要求系统 Node 与预先 `pnpm run build`;关窗强杀后端(逐事件持久化保证安全,但不是优雅 dispose);`DEEPSEEK_API_KEY` 仍来自仓库根 `.env`,没有应用内密钥录入。
