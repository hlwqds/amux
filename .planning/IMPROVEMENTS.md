# amux 工程优化方向 (Engineering Improvement Backlog)

> 项目:`amux` v0.3.0 — Rust 1.85 + ratatui 0.30 + alacritty_terminal 0.26 + axum 0.7
> 规模:18,741 行 Rust,28 个模块,核心是 TUI + PTY 多路复用 + HTTP/WebSocket 服务
> 适用对象:Claude Code / Codex / OMP 三个 AI 编码代理
> 文档维护:每完成一项打勾并标注 commit hash;每季度审视一次优先级

---

## 阅读说明

- **P0** = 阻塞问题,立即修(本周)
- **P1** = 质量债/性能,本月内消化
- **P2** = 功能扩展,下季度分批做
- **P3** = 生态与长期演进,1+ 年视角
- **状态**:`[ ]` 未开始 / `[~]` 进行中 / `[x]` 完成 / `[?]` 阻塞
- 每条引用**具体的文件:行号**,以便复核

---

## 一、质量债(P0/P1)

### 1. [x] P0 修复 `ProjectType::default()` 缺失
- **位置**:`src/discovery.rs:13`
- **修复**:已加 `#[derive(Default)]` + `#[default] Unknown`
- **阻塞**:`#2`

### 2. [?] P0 补 `render_terminal` 函数
### 2. [x] P0 补 `render_terminal` 函数
- **位置**:`src/app/ui.rs:932`
- **状态**:已实现 — `render_terminal` 渲染底部 shell PTY 分屏

### 3. [x] P1 删除死变体 `InputMode::DiffSelect`
- **位置**:`src/types.rs` — 已删除 `DiffSelect` 变体及 `session.rs` 引用
- **价值**:移除未分支覆盖的死代码,降低认知负担

### 4. [x] P1 把 `types.rs` 内的 60 行 `EVALUATION` 注释搬到 `docs/`
- **位置**:已移至 `docs/architecture-decisions/0001-inputmode-eval.md`,源文件仅保留一行引用

### 5. [x] P1 抽取 `apply_term_env` 消除 6 处重复
- **位置**:`Agent::apply_term_env()` — `types.rs:121`,6 处调用 + `pty.rs:309` 共享

### 6. [x] P1 提取 `available_agents()` 共享给 doctor/quick_doctor/util
- **位置**:`Agent::ALL` 常量 — `types.rs:92`,替换了 `util.rs`/`doctor.rs` 中 3 处硬编码列表
### 7. [ ] P2 拆分 `App` god-struct

- **现状**:`App` 把 `AppView` / `PtyManager` / `SessionStore` / `PopupState` / `ChainState` 五块全塞一起
- **方案**:
  - 短期:把 `ChainState` 的方法从 `handler.rs` 抽到 `src/chain.rs`(零风险)
  - 长期:把 `App` 拆成 `HasView` / `HasPtys` / `HasSessions` trait,handler 函数签名分小块
- **触发条件**:下次有人想动 `ChainState` 时,体验到"改 1 行跳 4 文件"的痛感,立即启动

---

## 二、性能与正确性(P1)

### 8. [x] P1 `SCROLLBACK_LINES` 从常量改为 `PtyHandle` 字段
- **位置**:`src/pty.rs:26` 改为 `pub const DEFAULT_SCROLLBACK_LINES: usize = 10000`,所有使用点已更新

### 9. [ ] P1 `discovery.rs` 解析 session 并行化
- **位置**:`src/discovery.rs:101-145` 串行遍历所有 JSONL 解析
- **问题**:冷启动 1000+ 历史 session 用户,首次 `discover_sessions` 需 500-800ms
- **修复**:加 `rayon = "1.10"` 依赖,`jsonl_files.par_iter().filter_map(...).collect()`
- **验收**:1000 个 session 冷启动 < 200ms

### 10. [ ] P2 `SearchIndex` 用 `hashbrown` + `roaring` 升级倒排
- **位置**:`src/search_engine.rs:27-31` 用 `std::HashMap<String, Vec<(String, usize)>>`
- **问题**:会话数过万,insert/remove 反复 realloc
- **修复**:
  - `hashbrown::HashMap` 替代 `std::HashMap`(`std` 内部已用它)
  - `roaring::RoaringTreemap` 替代 `Vec<(doc_id, freq)>` postings
  - 删文档从 O(N) 扫 postings 降到 O(1)
- **价值**:10k+ session 时搜索延迟稳定 <50ms

### 11. [ ] P1 `watch.rs` 限制递归深度避免 fd 耗尽
- **位置**:`src/watch.rs:55 watcher.watch(dir, RecursiveMode::Recursive)`
- **问题**:`~/.claude/projects/` 在 worktree-heavy 项目下可达几十万子目录,`notify` 在 Linux 每个 inotify watch 吃一个 fd
### 11. [x] P1 `watch.rs` 限制递归深度避免 fd 耗尽
- **位置**:`src/watch.rs:56` 改为 `RecursiveMode::NonRecursive`

### 12. [ ] P1 Web 模式 xterm.js 资源本地化
- **位置**:`src/server/static/index.html:7-8, 752-753` 通过 jsdelivr CDN 加载 xterm 5.3.0
- **问题**:
  - 国内网络 CDN 失败率高,启动即空白
  - 供应链审计风险
  - 隐含"amux 必须联网"依赖
- **修复**:
  1. `cargo` 引入 `static-files = "0.2"` 或 `axum-embed` 把 `xterm.js` / `xterm.css` / `xterm-addon-fit.js` 内嵌到二进制
  2. `index.html` 改成 `<script src="/static/vendor/xterm.js">`
  3. 文档加 "Web mode works offline" 说明
- **验收**:`amux --web` 在断网环境启动 + 终端能正常打开

### 13. [ ] P1 `PTY.write_input` 加背压
- **位置**:`src/pty.rs` `write_input` 直接灌
- **问题**:用户一次性 paste 1MB 文本 → PTY 阻塞 → amux 假死
- **修复**:`tokio::sync::mpsc::channel(256)`,溢出后 `try_send` 失败时:
  - 选项 A:返回 backpressure error,UI 显示 status
  - 选项 B:drop 后续字节 + 状态栏提示"N bytes dropped"
- **验收**:粘贴 1MB 文本不卡死主进程,状态栏给出明确反馈

### 14. [ ] P1 启动时检测未跟踪的 build 失败
- **位置**:本期发现 — git status 有 7 个未提交改动 + 主干编译失败
- **修复**:
  - `ci.yml` 加 `cargo build --release` 必跑步骤,失败即红
  - 或在 `amux doctor` 里加一段 `git status` 检测(如果当前是 git 仓库)
- **价值**:防止"本地编译过没跑"→ "push 上去 CI 红"→ "回滚或修"循环

---

## 三、功能扩展(P2)

按"对开发者工作流的价值"排序。每条带 **A. 协作 / B. TUI / C. 集成 / D. 可观测** 标签。

### 协作与上下文

### 15. [ ] P2 LLM 结构化抽取项目心智模型
- **现状**:`src/knowledge.rs:8 WorkspaceKnowledge` 收集 `architecture/key_files/tech_stack/known_issues`,但 `merge_from_session` 全是字符串关键词正则(`knowledge.rs:96-103`)
- **方案**:
  1. 每次 PTY `Completed` 后,把会话输出 pipe 给 `claude --print "extract: architecture/key_files/tech_stack/known_issues from the following: ..."`(零新依赖)
  2. 解析结构化 JSON 后写回 `knowledge.json`
  3. 启动新 PTY 时把结构化知识作为 system prompt 一段注入
- **价值**:从"杂乱关键词表"升级到"可被引用的项目心智模型"

### 16. [ ] P2 每 PTY 侧栏实时显示相关历史会话
- **现状**:`SearchIndex`(纯 BM25,352 行)只服务 `SemanticSearch` 一个 popup
- **方案**:
  1. 每个 PTY 拿到自己的 last_user_message,跑 BM25 找 top-3
  2. 在 Chat 视图右/下角 20% 区域显示"similar past sessions"
  3. 点击 → resume 那个 session
- **价值**:比 Claude 自带的 `--resume --continue` 强很多(可跨 agent 类型)

### 17. [ ] P2 asciinema 格式录制会话行为
- **现状**:`PtySlot.last_recording_at`(`src/app/mod.rs:391`)字段已存在但没用到
- **方案**:
  1. PTY spawn 时创建 `~/.local/share/amux/recordings/<session>.cast`
  2. 写 asciinema v2 格式 header + 事件流
  3. `amux replay <session_id>` 启动 xterm.js 播放
  4. 配合 `asciinema play` 形成可分享的 agent 行为录像
- **价值**:协作/教学/复盘

### 18. [ ] P2 `ChainStep` 加 `expected_output_schema` + 并发 Vote
- **现状**:`src/chain.rs` 是"上一步输出 → 下一步 prompt"({prev_output} 替换)
- **方案**:
  - `ChainStep` 加 `expected_output_schema: Option<Value>` 字段(JSON Schema)
  - 步骤完成后验证输出符合 schema,失败 → 重试 or 跳到 error handler
  - `Chain` 加 `mode: Sequential | Parallel` 模式,`Parallel` 用 `tokio::join!` 同时启动多 agent,`Vote` 模式由 LLM 评估器打分选 best
- **价值**:多 agent 协商从"链式串行"升级为"真正的并发协作"

### TUI 体验

### 19. [ ] P2 TUI 鼠标拖拽多分栏
- **现状**:`src/app/ui.rs:73-76` 强制 30/70 横向分屏,改完编译失败
- **方案**:
  - 接 `crossterm::event::MouseEvent`(已依赖)
  - 拆分栏之间加 drag handle,实时调整 Constraint::Percentage
  - 支持 2-3 栏布局,每栏独立 PTY
- **价值**:同时看 Claude 写代码 + Codex 跑测试

### 20. [ ] P2 `Timeline` 与 `git log --graph --decorate` 合并可视化
- **现状**:`InputMode::Timeline` + `extract_branch_points` 已有(`src/discovery.rs`),但只是"事件列表"
- **方案**:渲染 `git log --graph --oneline --decorate --color=always`,叠加 `Event::AgentCompleted` 标记
- **价值**:看到 commit 落点 = 看到 agent 改了什么

### 21. [ ] P2 scrollback 增量搜索增强
- **现状**:`src/app/handler.rs:1313` 基础高亮,不支持正则/多行/大小写敏感
- **方案**:
  - 切换 `Ctrl+F` 打开正则模式(regex crate)
  - 选项面板:`a` 大小写敏感 / `w` 整词 / `r` 正则
  - 跨多行匹配:把 `Vec<String>` 当文本,`(.|\\n)*?` 风格
- **价值**:vscode Ctrl+Shift-F 体验

### 22. [ ] P2 跨会话 pass rate 折线图
- **现状**:`CheckStatus` 已有 `Pending/Running/Passed/Failed`(`src/types.rs`),但**没有跨会话对比**
- **方案**:
  - `data_dir/stats/<YYYY-MM-DD>.json` 每天聚合:`{ passed: n, failed: m, avg_duration: s }`
  - 新 popup:`InputMode::Stats`,`ratatui::widgets::Chart` 渲染 30 天折线
  - 顺手加 token 用量折线
- **价值**:一眼看到"这周 Claude 一次通过率在跌"

### 23. [ ] P1 fuzzy picker 全覆盖
- **现状**:`fuzzy-matcher` 已在 `Cargo.toml` 依赖里,但 `util.rs` 用得不多
- **方案**:把 agent 列表 / template 列表 / theme 列表 / plugin 列表**全部接入** fuzzy picker,统一 `InputMode::FuzzyPicker { kind: PickerKind }`
- **价值**:任何列表操作都是同一种体验,降低学习成本

### 集成与生态

### 24. [ ] P2 `amux attach` 接入 tmux
- **现状**:`pty.rs` 已经是真 PTY
- **方案**:`amux attach` 子命令调 `tmux -L amux new-session -d 'amux tui'`,SSH 进去后能 `amux attach` 接回
- **价值**:远程开发 + 长期任务

### 25. [ ] P2 `amux hook` 触发外部 CI
- **现状**:`Plugin.hooks: Vec<String>` 字段已存在(`src/types.rs:268`),但**没看到任何代码触发**
- **方案**:
  1. `amux hook complete <session_id> [--exit-code N]` 子命令
  2. 内部 PTY Completed 时自动触发 `Plugin.command`(`Config` 已支持)
  3. GitHub Actions / GitLab CI 可订阅"agent 完成"事件
- **价值**:agent 完成 → 自动跑测试 → 通知(单条命令)

### 26. [ ] P2 把 `server` 包装为 MCP 服务
- **现状**:`src/server/api.rs:114-145` 已有 pty_input/pty_resize 端点
- **方案**:用 `rmcp` crate 把现有 endpoint 转 MCP 工具集:
  - `mcp__amux__list_sessions`
  - `mcp__amux__send_input(pty_id, text)`
  - `mcp__amux__attach_pty(pty_id)`
  - `mcp__amux__search_history(query)`
- **价值**:**Claude Code 在跑的同时,通过 MCP 操作另一个 PTY 上的 Codex 跑测试** — 这才是真正的多 agent 并发协作

### 27. [ ] P2 `Config` 支持 `config.d/` 目录 + `imports` 字段
- **现状**:`Config`(`src/types.rs:286`)21 个字段全部硬塞 `config.json`
- **方案**:
  - 启动时 `load_config()` 读 `config.json` + `config.d/*.json`(按文件名字母序合并)
  - `Config` 加 `imports: Vec<String>` 字段,允许项目级 `.amux.json` 继承全局基线
- **价值**:monorepo 子项目能共享基线配置

### 28. [ ] P2 `.amux.json` 用 `jsonschema` 校验
- **现状**:`load_project_config`(`src/config.rs:72`)解析失败就 `unwrap_or_default()` 静默吞错
- **方案**:
  1. 内嵌 `.amux.json` 的 JSON Schema
  2. 加载时 `jsonschema::validate()`,失败 → 启动报错精确到行号
- **价值**:用户配错不再神秘,新手上手成本下降

### 可观测

### 29. [ ] P2 30 天 token 用量柱状图
- **现状**:`src/budget.rs` 已有日/周 token limit,`TokenStats` popup 存在但**只显示当前值**
- **方案**:用 `ratatui::widgets::Chart` 渲染 30 天柱状图,无新依赖
- **价值**:直观看到"这周花得比上周多 40%"

### 30. [ ] P2 会话时长/成功率仪表盘
- **方案**:与 #22 共用底层 stats,加 dashboard popup,`BarChart` 多指标
- **价值**:启动后看到"今早 Claude session 平均 4m23s, 87% 一次通过"

### 31. [x] P1 接 `tracing` 写结构化日志
- **位置**:`Cargo.toml` 加 `tracing` + `tracing-subscriber`,`main.rs` 初始化,`pty.rs`/`watch.rs`/`headless.rs`/`worktree.rs`/`server/mod.rs` 已加 `info!`/`warn!`/`error!` 调用
- **验收**:`RUST_LOG=debug cargo run` 输出结构化日志

### 32. [ ] P2 `amux doctor --fix` 自动修复
- **现状**:`src/doctor.rs` 只报告问题(`CheckResult.fix_hint: Option<String>`),不能自动修
- **方案**:
  1. 现有 fix_hint 升级为 `Fix { kind: AutoFix, command: String, needs_confirm: bool }`
  2. `--fix` 参数 + 二次确认 prompt(防误操作)
  3. 覆盖:no agent CLI → `npm i -g @anthropic-ai/claude-code`、缺 `config.json` → 写默认、缺 `data_dir` → mkdir
- **价值**:新用户第一次跑 `amux doctor --fix` 就能用

---

## 四、生态与长期演进(P3)

### 33. [ ] P3 crates.io 发布准备
- **现状**:`Cargo.toml` 缺 `description` / `license` / `repository` / `keywords`,纯本地构建
- **修复**:
  ```toml
  [package]
  description = "TUI multiplexer for AI coding agents (Claude Code, Codex, OMP)"
  license = "MIT OR Apache-2.0"
  repository = "https://github.com/<org>/amux"
  keywords = ["tui", "ai", "claude", "codex", "pty", "multiplexer"]
  categories = ["command-line-utilities", "development-tools"]
  readme = "README.md"
  ```
- **验收**:`cargo publish --dry-run` 跑通

### 34. [ ] P3 GitHub Actions CI
- **现状**:`.github/` 目录存在但空
- **方案**:
  - `.github/workflows/release.yml`:tag push → 交叉编译 linux-x64/aarch64 + macos-x64/aarch64 → release tarball
  - 集成 `cargo-deny` / `cargo-audit` 检查依赖

### 35. [ ] P3 `CHANGELOG.md` 0.x 跟踪
- **现状**:`git log` 有 200+ commits 但无 changelog
- **方案**:`CHANGELOG.md` 用 [Keep a Changelog](https://keepachangelog.com/) 格式,0.x 阶段用 `[Unreleased]` 段记录

### 36. [ ] P3 补 `search_engine::score_bm25` 单元测试
- **现状**:`src/search_engine.rs:104+` 有 100+ 行测试,但**没看到 `score_bm25` 核心打分函数的明确断言**
- **方案**:为 IDF = log((N - df + 0.5)/(df + 0.5)) 等关键路径加单测

### 37. [ ] P3 补 `discovery` cache 命中逻辑测试
- **现状**:`src/discovery.rs` 测了 `parse_gsd_session` 的各种 JSON,**没测 mtime 缓存命中逻辑**
- **方案**:mock mtime,断言 `discover_sessions_cached` 在 mtime 未变时直接返回

### 38. [ ] P3 补 `pty.rs` 集成测试
- **现状**:`src/pty.rs` 0 个测试,这是最 critical 的模块
- **方案**:
  - `cargo test --test pty_integration` spawn 真 shell,验证输出能拿回
  - 用 `portable-pty` 的 mock 跑 daemon 关闭 / 大输出 / CJK 路径

### 39. [ ] P3 `app/handler/ui` 集成测试 + `insta` 快照
- **现状**:`src/app/mod.rs:3745 行` + `handler.rs:1313 行` + `ui.rs:3788 行`**0 个集成测试**
- **方案**:`tests/integration/` 用 `insta` 快照渲染输出,主流程 key 路径各 1 个快照

### 40. [ ] P2 `PtyState` 改 `DashMap` 分片锁
- **现状**:`parking_lot::Mutex<HashMap<id, RegisteredPty>>` 在 `src/server/mod.rs:31-36` 是单点锁
- **问题**:10 个并发 WS 连接就有竞争
- **修复**:换 `dashmap::DashMap`,或 `Arc<RwLock<HashMap>>` + 一致性哈希分片

### 41. [ ] P2 unset 环境变量列表迁到配置
- **现状**:`types.rs:127-129` 等处硬编码 `KITTY_WINDOW_ID` / `GHOSTTY_BIN_DIR` / `TERM_PROGRAM` / `ALACRITTY_WINDOW_ID`
- **方案**:`Config.unset_env: Vec<String>`,用户能加自己的(`JEST_WORKER_ID`、`CLAUDE_CODE_ENTRYPOINT`)

### 42. [ ] P3 补 `CONTRIBUTING.md` / `ARCHITECTURE.md` / `TROUBLESHOOTING.md`
- **现状**:`docs/` 整个目录**只有 2 个文件**(`chains.md` + `config.md`),项目有 28 个模块
### 41. [x] P2 unset 环境变量列表迁到配置
- **位置**:`Config.unset_env: Vec<String>` 加于 `types.rs:311`,`Agent::DEFAULT_UNSET_ENV` 常量 + `apply_term_env_with_extra()` 方法
  - `ARCHITECTURE.md`:模块依赖图、关键数据流
  - `TROUBLESHOOTING.md`:常见问题(PTY 假死、token 计费不准、agent 找不到 session 目录)

### 43. [ ] P3 修正 GSD 文档与代码不一致
### 43. [x] P3 修正 GSD 文档与代码不一致
- **位置**:`docs/chains.md` 和 `docs/config.md` — 已移除不存在的 `Gsd` agent 引用

---

## 五、推荐的执行顺序

| 阶段 | 任务 | 预计依赖 | 验收标准 |
|------|------|----------|----------|
| **今天** | #1 (另一个 agent 改完后) | #2 | `cargo build --release` 成功 |
| **今天** | #3, #4, #5, #6 | 无 | 7 处 `PtyState::*` / 60 行注释 / 18 行重复全部消失 |
| **本周** | #8, #11, #12, #13, #14, #31 | tracing 引入;xterm 资源 | 断网启动 web 模式正常 |
| **本月** | #9, #23, #33, #34, #35 | #34 装好 runner | 第一次 `cargo publish` 跑通,CI 绿 |
| **下季度** | #15, #16, #26, #19 | 多数需要重 UI 工作 | MCP 服务 + 多分栏可用 |
| **长期** | #7, #10, #40, #42, #43 | 架构债 | god-struct 拆完,docs 完整 |

---

## 维护规则

1. **每完成一项**:把 `[ ]` 改 `[x]`,在 commit message 末尾加 `Refs: .planning/IMPROVEMENTS.md#N`
2. **每发现新问题**:追加到对应 phase 末尾,新 P 等级
3. **每季度审视**:把已完成的 `[x]` 折叠到 "历史归档"段,空出主表
4. **优先级重排**:仅在 P0/P1 全部清空后,或用户要求时
5. **冲突解决**:如果某个 P3 任务实现上需要 P0/P1 基础设施(如 MCP 暴露前先有 tracing),立即升级它的优先级
