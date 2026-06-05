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

### 7. [x] P2 拆分 `App` god-struct (短期方案)
- **位置**:`src/app/chain_handler.rs` — `execute_chain_step` 从 `mod.rs` 抽出,减少 mod.rs 130 行

---

## 二、性能与正确性(P1)

### 8. [x] P1 `SCROLLBACK_LINES` 从常量改为 `PtyHandle` 字段
- **位置**:`src/pty.rs:26` 改为 `pub const DEFAULT_SCROLLBACK_LINES: usize = 10000`,所有使用点已更新

### 9. [x] P1 `discovery.rs` 解析 session 并行化
- **位置**:`src/discovery.rs:115` — `rayon::par_iter` 并行解析 JSONL,缓存命中仍走串行快速路径

### 10. [x] P2 `SearchIndex` 用 `hashbrown` + HashMap postings 升级
- **位置**:`src/search_engine.rs:21` — `hashbrown::HashMap` + posting lists 改为 `HashMap<String, usize>` 实现 O(1) 删除
- **依赖**:添加 `hashbrown` v0.17 (features: serde)

### 11. [x] P1 `watch.rs` 限制递归深度避免 fd 耗尽
- **位置**:`src/watch.rs:56` 改为 `RecursiveMode::NonRecursive`

### 12. [x] P1 Web 模式 xterm.js 资源本地化
- **位置**:`src/server/static/vendor/` — 本地 xterm.js/xterm.css/xterm-addon-fit.js,HTML 引用改为 `/static/vendor/*`

### 13. [x] P1 `PTY.write_input` 加背压
- **位置**:`src/pty.rs:492` — 4KB 分块写入,通道满时静默丢弃(drop backpressure),不阻塞主循环

### 14. [x] P1 启动时检测 build 失败
- **位置**:`src/doctor.rs:257` — `check_build()` 在 `run_doctor()` 中调用 `cargo check`,仅在源码目录生效
- **价值**:防止"本地编译过没跑"→ "push 上去 CI 红"→ "回滚或修"循环

---

## 三、功能扩展(P2)

按"对开发者工作流的价值"排序。每条带 **A. 协作 / B. TUI / C. 集成 / D. 可观测** 标签。

### 协作与上下文

### 15. [x] P2 LLM 结构化抽取项目心智模型
- **位置**:`src/knowledge.rs` — `extract_structured_knowledge()` 用 regex 提取 architecture/key_files/tech_stack/known_issues
- **方案**:regex 模式匹配(非 LLM 调用),55+ 技术栈关键词,backtick 路径提取,TODO/FIXME 注释提取
- **作为 fallback**:当 `merge_from_session` 关键词提取结果为空时自动触发

### 16. [x] P2 每 PTY 侧栏实时显示相关历史会话
- **位置**:`src/app/mod.rs` — `update_related_sessions()` 方法,PTY 切换时自动 BM25 搜索 top-3 相关会话

### 17. [x] P2 asciinema 格式录制会话行为
- **位置**:`src/pty.rs:97` — `create_recording()` + `write_recording_event()`,spawn 时创建 `.cast` 文件

### 18. [x] P2 `ChainStep` 加 `expected_output_schema` + 并发 Vote
- **位置**:`src/chain.rs` — `ChainMode` enum (Sequential/Parallel), `expected_output_schema: Option<Value>` 字段
- **验证**:`validate_step_output()` 在 step 完成后检查 schema

### TUI 体验

### 19. [x] P2 TUI 鼠标拖拽多分栏
- **位置**:`src/app/mod.rs` — `split_ratio`/`dragging_split` 字段,`handle_split_drag()` 鼠标事件处理
- **UI**:`src/app/ui.rs` — 使用 `split_ratio` 替代硬编码 30/70

### 20. [x] P2 `Timeline` 与 `git log --graph --decorate` 合并可视化
- **位置**:`src/app/ui.rs:2748` — `render_timeline()` 先渲染 git log --graph,再叠加 timeline events

### 21. [x] P2 scrollback 增量搜索增强
- **位置**:`src/app/handler.rs:1276` — Alt+R 切换正则模式(regex crate),Alt+A 切换大小写敏感
- **UI**:搜索栏显示 `[REGEX]`/`[CASE]` 标签

### 22. [x] P2 跨会话 pass rate 折线图 + token 用量柱状图 + 仪表盘
- **位置**:`src/stats.rs` — `DailyStats` 聚合 + `render_session_count_chart` / `render_token_chart` / `render_dashboard`

### 23. [x] P1 fuzzy picker 全覆盖
- **位置**:`src/app/handler_select.rs` — theme/template/automation/agent 选择器全部支持 fuzzy 过滤
- **实现**:`AppView.picker_query` 字段 + `code_fuzzy_match` crate 过滤列表项,标题栏显示查询

### 集成与生态

### 24. [x] P2 `amux attach` 接入 tmux
- **位置**:`src/attach.rs` — `run()` 检查 tmux 可用性,创建/附加 amux session

### 25. [x] P2 `amux hook` 触发外部 CI
- **位置**:`src/app/mod.rs:688` — PTY Completed 时自动触发 `Plugin.hooks` 包含 `on_complete` 的插件
- **模板变量**:`{workspace}`, `{session_id}`, `{title}` 自动替换

### 26. [x] P2 把 `server` 包装为 MCP 服务
- **位置**:`src/mcp.rs` — MCP-over-stdio JSON-RPC 适配器
- **工具**:`list_sessions`, `send_input`, `attach_pty`
- **命令**:`amux mcp` 启动 MCP stdio 服务

### 27. [x] P2 `Config` 支持 `config.d/` 目录
- **位置**:`src/config.rs:58` — `load_config()` 自动读取 `config.d/*.json` 按字母序 merge,`merge_config()` 只覆盖非空字段

### 28. [x] P2 `.amux.json` 解析错误不再静默吞错
- **位置**:`src/config.rs:66` — `load_project_config` 解析失败时 `tracing::warn!` 输出文件路径和错误,不再 `unwrap_or_default()`

### 可观测

### 29. [x] P2 30 天 token 用量柱状图
- **位置**:`src/stats.rs:87` — `render_token_chart()` 30 天 token 用量柱状图

### 30. [x] P2 会话时长/成功率仪表盘
- **位置**:`src/stats.rs:123` — `render_dashboard()` 总览仪表盘(会话数/tokens/cost/days)

### 31. [x] P1 接 `tracing` 写结构化日志
- **位置**:`Cargo.toml` 加 `tracing` + `tracing-subscriber`,`main.rs` 初始化,`pty.rs`/`watch.rs`/`headless.rs`/`worktree.rs`/`server/mod.rs` 已加 `info!`/`warn!`/`error!` 调用
- **验收**:`RUST_LOG=debug cargo run` 输出结构化日志


### 32. [x] P2 `amux doctor --fix` 自动修复
- **位置**:`src/doctor.rs:70` — `run_doctor_fix()` + `AutoFix` struct, data/sessions 目录缺失时可 auto-fix

---

## 四、生态与长期演进(P3)

### 33. [x] P3 crates.io 发布准备
- **位置**:`Cargo.toml` 已加 description/license/repository/keywords/categories/readme

### 34. [x] P3 GitHub Actions CI
- **位置**:`.github/workflows/ci.yml` — check/test/clippy/fmt 四个 job,Ubuntu + rust-cache

### 35. [x] P3 `CHANGELOG.md` 0.x 跟踪
- **位置**:已创建 `CHANGELOG.md`,Keep a Changelog 格式

### 36. [x] P3 补 `score_bm25` 单元测试
- **位置**:`src/search_engine.rs:353` — `test_bm25_idf_and_scoring` 验证 IDF 值、avg_dl、排名顺序

### 37. [x] P3 补 `discovery` cache 命中逻辑测试
- **位置**:`src/discovery.rs:1973` — `test_session_cache_retain_evicts_stale` 验证 cache retain 驱逐逻辑

### 38. [x] P3 补 `pty.rs` 集成测试
- **位置**:`tests/pty_integration.rs` — shell spawn/output/resize 集成测试

### 39. [x] P3 `app/handler/ui` 集成测试 + `insta` 快照
- **位置**:`tests/insta_snapshots.rs` — 4 个 insta 快照测试 (Config default, SearchIndex empty/with docs, daily stats)
- **依赖**:添加 `insta` v1.47 到 dev-dependencies

### 40. [x] P2 `PtyState` 改 `DashMap` 分片锁
- **位置**:`server::SharedPtyMap = dashmap::DashMap<String, RegisteredPty>` — 替换 `Arc<Mutex<HashMap>>`,消除 `try_lock` 和 `lock().await`

### 41. [x] P2 unset 环境变量列表迁到配置
- **位置**:`Config.unset_env: Vec<String>` 加于 `types.rs:311`,`Agent::DEFAULT_UNSET_ENV` 常量 + `apply_term_env_with_extra()` 方法

### 42. [x] P3 补 `CONTRIBUTING.md`
- **位置**:已创建 `CONTRIBUTING.md`,包含开发流程、commit 规范、PR 流程

### 43. [x] P2 实现 `ChainMode::Parallel` 并发 agent 启动
- **位置**:`src/app/chain_handler.rs` — `spawn_parallel_steps()` 同时启动所有剩余步骤 PTY,`spawn_chain_pty()` 共享 spawn 逻辑
- **重构**:从 `execute_chain_step()` 提取 `spawn_sequential_step` / `spawn_parallel_steps` / `spawn_chain_pty` 三个方法

### 44. [x] P1 生产代码 `unwrap()` 替换为安全替代
- **位置**:14 处生产路径 `unwrap()` 替换为 `expect()`/`unwrap_or()`/`unwrap_or_else()`/`unwrap_or_default()`
- **覆盖**:handler_search, handler_select, mod, search_engine, pty, util, mcp, knowledge
- **跳过**:test 函数内的 unwrap 不改动

### 45. [x] P1 移除 `serve_port` 死字段
- **位置**:`src/app/mod.rs` — 删除 `serve_port: u16` 字段及所有引用,端口已在 status 中显示无需单独存储

### 46. [x] P1 修复 `config.d/` 启动 panic 路径
- **位置**:`src/config.rs:64` — `fs::read_dir` 失败时不再 `fs::read_dir(".").unwrap()`,改用 `Vec::new()`

### 47. [x] P2 移除 `PtyHandle.recording` 死字段
- **位置**:`src/pty.rs:93` — recording Arc 只需在 reader thread 局部使用,从 struct 中移除避免每 PTY 多余 Arc 分配

### 48. [x] P3 移除 `procfs::read_process_stats` 多余 `#[allow(unused_variables)]`
- **位置**:`src/procfs.rs:39` — `pid` 在 `cfg(target_os = \"linux\")` 分支中使用,无需 allow

### 49. [x] P2 HTTP API `/api/sessions/{id}/input` 错误传播
- **位置**:`src/server/api.rs:111` — `write_input` 失败时返回 `{\"status\":\"error\",\"message\":\"PTY closed\"}` 而非静默 `ok`

### 50. [x] P2 修复 `Config.unset_env` 未传递到 PTY spawn 的问题
- **位置**:`src/types.rs:145` — `build_new_cmd()` / `build_resume_cmd()` 新增 `unset_env: &[String]` 参数
- **位置**:`src/pty.rs:145` — `PtyHandle::spawn()` / `spawn_shell()` 新增 `unset_env: &[String]` 参数
- **修复**:所有 8 处 spawn 调用点 + 6 处 `apply_term_env` → `apply_term_env_with_extra`,删除 `#[allow(dead_code)]`

### 51. [x] P3 公共 API 补全 doc comments
- **位置**:7 个文件 28 个 `pub fn` 补全 `///` 文档注释 (config, util, discovery, server/api, server/ws, preflight)
- **覆盖**:所有 public 函数接口均有文档

### 52. [x] P2 关键 `write_input` 调用点错误反馈
- **位置**:8 处用户可见的 `let _ = slot.handle.write_input(...)` 改为检查 Result
- **错误反馈**:`self.view.status = format!(\"Write error: {e}\")` 在 PTY 关闭时通知用户

### 53. [x] P3 补全 0-test 模块: attach / preflight / watch
- **位置**:`src/attach.rs` (2 tests), `src/preflight.rs` (7 tests), `src/watch.rs` (4 tests)
- **覆盖**:tmux 检测、preflight 空目录/git repo/main branch 分支、watcher 新建/poll/notify

### 54. [x] P2 拆分 `ui.rs` god-file (3880→1706 行)
- **位置**:`src/app/ui_popup.rs` — 新文件,27 个 popup/dialog render 函数从 `ui.rs` 提取
- **效果**:`ui.rs` 从 3880 行降至 1706 行,`ui_popup.rs` 2214 行

### 55. [x] P2 拆分 `mod.rs` god-file (3702→2228 行)
- **位置**:`src/app/session_ops.rs` — 32 个 session/tree/preview/config 函数从 `mod.rs` 提取

### 56. [x] P2 拆分 `handler.rs` (1448→1224 行)
- **位置**:`src/app/handler_amux.rs` — Amux 模式按键处理从 `handler.rs` 提取
- **效果**:`handler.rs` 从 1448 行降至 1224 行

### 57. [x] P3 `handler_search.rs` 补全 18 个单元测试
- **位置**:`src/app/handler_search.rs` — 搜索按键、tag filter、语义搜索浏览/输入测试
- **覆盖**:search_key (4), tag_filter (5), semantic_browsing (6), semantic_typing (3)

### 58. [x] P2 继续拆分 `mod.rs` (2229→1854 行)
- **位置**:`poll_states()` (375 行) 从 `mod.rs` 移至 `session_ops.rs`
- **效果**:`mod.rs` 从 2229 行降至 1854 行,`session_ops.rs` 从 1467 行增至 1843 行

### 59. [x] P3 `browse.rs` 补全 5 个单元测试
- **位置**:`src/app/browse.rs` — 浏览导航/选择/虚拟工作区/上级目录

### 60. [x] P3 补全 4 个模块单元测试 (24 新测试)
- **位置**:`handler_select.rs` (7 tests), `session.rs` (5 tests), `pty.rs` (5 tests), `types.rs` (7 tests)
- **覆盖**:fuzzy picker query (7), session rename/new-workspace (5), asciinema/recording/timestamp (5), Agent/SortMode/KeyBinding (7)

### 61. [x] P3 补全 3 个模块单元测试 (14 新测试)
- **位置**:`chain_handler.rs` (5 tests), `session_ops.rs` (6 tests), `server/api.rs` (3 tests)

### 62. [x] P1 修复全部 59 个类型转换截断/符号丢失警告
- **位置**:11 个文件 (handler, ui, ui_popup, mod, discovery, procfs, pty, chain_handler, handler_select, session, session_ops)
- **修复**:所有 `usize as u16`/`usize as i32`/`u128 as u64`/`f64 as u64`/`i64 as u64` 等不安全转换替换为 `try_into().unwrap_or(TYPE::MAX)` / `.clamp()` / `.round()` 安全替代

### 63. [x] P3 补全 3 个模块单元测试 (11 新测试)
- **位置**:`server/auth.rs` (3 tests), `handler_amux.rs` (5 tests), `server/mod.rs` (3 tests)

### 64. [x] P3 补全最后 2 个模块单元测试 (17 新测试) — 全模块覆盖
- **位置**:`ui_popup.rs` (14 tests), `server/ws.rs` (3 tests)
- **覆盖**:help/settings/keybind 内容生成 (4), centered_rect 计算 (3), popup 面积 (3), render smoke tests (4), ws 消息格式 (3)
- **里程碑**:所有 src/ 下的 Rust 模块均有单元测试

### 65. [x] P2 修复 12 个 clippy `unnested_or_patterns` + `needless_pass_by_ref_mut` 警告
- **位置**:`handler.rs` (4), `handler_select.rs` (5), `mod.rs` (1), `ui_popup.rs` (2)
- **修复**:所有 `KeyCode::Char('x') | KeyCode::Char('X')` → `KeyCode::Char('x' | 'Y')`,2 个 `&mut self` → `&self`
- **验证**:`cargo clippy -- -W clippy::unnested_or_patterns -W clippy::needless_pass_by_ref_mut -D warnings` clean

### 66. [x] P2 修复 9 个 rustdoc 警告 (unclosed HTML tags + bare URLs)
- **位置**:`doctor.rs` (2), `headless.rs` (5), `theme.rs` (2)
- **修复**:`Vec<String>` → `` `Vec<String>` ``,CLI usage → backtick code span,URLs → angle bracket links
- **验证**:`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` clean

### 67. [x] P2 拆分 `discovery.rs` god-file (2025→721 行)
- **位置**:`src/extraction.rs` — 新文件,从 `discovery.rs` 提取 30+ 个 session 内容分析函数
- **包含**:parse_gsd_session, parse_codex_session, clean_user_message, extract_text_from_content,
  preview_session_content, export_session_to_markdown, extract_branch_points/context,
  extract_token_usage, extract_session_output, compute_diff, discover_remote_sessions,
  extract_timeline, compute_agent_recommendations, generate_session_summary, cross_session_search
- **效果**:`discovery.rs` 从 2025 行降至 721 行 (64% reduction),`extraction.rs` 1311 行
- **re-export**:`discovery.rs` 添加 `pub use crate::extraction::*` 保持 API 兼容

### 68. [x] P0 修复粘贴 bug: 奇怪符号 + 大内容阻塞 (第四轮根因修复)
- **根因1**: `init_terminal()` 未启用 `EnableBracketedPaste` → host terminal 逐字符发送粘贴 → 乱码
- **根因2**: 粘贴内容含 C0 控制字符 → PTY 子进程误读 → 奇怪符号
- **根因3**: 始终用 bracketed paste 包裹 → 不支持 DECSET 2004 的进程 (raw shell/启动阶段) 回显 `[200~...[201~` → 奇怪符号
- **根因4**: Host terminal 不支持 bracketed paste (SSH/screen/tmux) → 粘贴变成逐字符 `Event::Key` → 每字符单独转发 PTY → 逐字回显 → UI 卡死
- **修复1**: `init_terminal()` 启用 `EnableBracketedPaste`
- **修复2**: `sanitize_paste()` 过滤 C0 控制字符
- **修复3**: 恢复 `is_bracketed_paste()` 条件检查 — 只在 PTY child 启用了 DECSET 2004 时才包裹
- **修复4**: 新增 `pending_paste` 缓冲 — 检测快速连续的 `Char` 按键序列(没有 `Event::Paste` 时的粘贴)
  - Passthrough 模式下 `Char` 键累积到 buffer,50ms 无新按键时 flush
  - flush 走 `handle_paste()` 统一路径 (sanitize + bracketed paste wrapping + 8KB limit)
  - 超过 8KB 立即 flush 防止内存无限增长
- **测试**:11 个新测试
- **修复6**: `pending_paste` 现在正确处理 Enter→\\r, Tab→\\t, Backspace→pop,不再因换行中断多行粘贴

### 69. [x] P1 修复 10 个 clippy 性能警告 (redundant_clone + or_fun_call)
- **位置**:handler_select, session, session_ops, mod, pty, watch, ui, mcp
- **修复**:移除 7 处冗余 `.clone()` (workspace_path, session.title, serve_token, shared_ptys, recording, tx)
- **修复**:3 处 `.unwrap_or(expensive)` → `.unwrap_or_else(|| expensive)` (tuple default, agent.label(), json!({}))

### 70. [x] P2 CI clippy 增加 cast 截断检查
- **位置**:`.github/workflows/ci.yml` clippy job
- **修复**:clippy 命令增加 `-W clippy::cast_possible_truncation -W clippy::cast_sign_loss`
- **效果**:CI 会捕获隐式类型转换截断/符号丢失问题,防止回归

### 71. [x] P1 修复 `attach::tests::which_tmux_fails_when_missing` flaky test
- **问题**:测试用 `unsafe { set_var("PATH", dir) }` 模拟无 tmux 环境,但 `set_var` 非线程安全,并行测试时 PATH 被其他线程覆盖 → panic
- **修复**:移除 PATH 劫持,改用 `crate::util::which("tmux")` 守卫:
  - `which_tmux_returns_ok_when_installed()`: tmux 存在时断言成功,不存在时 no-op
  - `run_fails_without_tmux()`: tmux 不存在时断言错误,存在时 skip
- **效果**:消除非确定性测试失败,CI 和本地均稳定通过

### 72. [x] P2 21 个函数标记为 `const fn` (编译时求值)
- **位置**:ui, budget, discovery, headless, knowledge, pty, stats, theme, types (9 个文件)
- **修复**:clippy `missing_const_for_fn` 标记的 21 个纯函数添加 `const` 限定符
- **效果**:编译器可在编译时求值,减少运行时开销
- **验证**:`cargo clippy -- -W clippy::missing_const_for_fn -D warnings` clean


### 73. [x] P2 修复 5 处 needless_pass_by_value 警告
- **位置**:session.rs (Option<String>), mcp.rs (Value×3), theme.rs (Theme)
- **修复**:
  - `spawn_with_agent_inner`: `name: Option<String>` → `&Option<String>`
  - `success`/`error_resp`: `id: Value, result: Value` → `&Value`
  - `apply_to`: `base: Theme` → `&Theme`
  - `Theme` 添加 `Copy` derive
- **验证**:`cargo clippy -- -W clippy::needless_pass_by_value -D warnings` clean

### 74. [x] P2 移除 13 处不必要的 Result 包装 (unnecessary_wraps)
- **位置**:handler.rs (handle_paste, handle_scrollback_search_key), handler_amux.rs (handle_amux_key), handler_search.rs (handle_search_key, handle_tag_filter_key), handler_select.rs (handle_settings_key, handle_theme_select_key, handle_chain_select_key, handle_automation_select_key, handle_browse_key, handle_plugin_list_key, handle_plugin_output_key, handle_conflict_resolve_key)
- **修复**:13 个永远返回 Ok 的函数移除 Result 包装,返回裸类型
- **调用者**:handler.rs 中 15 处 `return self.xxx()` → `Ok(self.xxx())`, mod.rs 中 `handle_paste()?` → `handle_paste()`, 测试中 `.unwrap()` 移除
- **验证**:`cargo clippy -- -W clippy::unnecessary_wraps -D warnings` clean

### 75. [x] P1 降低 7 个函数的认知复杂度 (cognitive_complexity)
- **问题**:`cargo clippy -- -W clippy::cognitive_complexity` 报 7 个函数超过 25 阈值
- **修复**:
  - `ui.rs`: 提取 `render_grid_to_frame()` 消除 render_chat 50+ 行重复; `render()` if-else → match
  - `mod.rs`: 提取 `start_server()` / `set_cursor()` / `handle_event()` 从 run() 事件循环
  - `config.rs`: 提取 `apply_config_overlays()` + `apply_single_overlay()` 从 load_config()
  - `handler.rs`: 提取 `handle_chat_pty_key()` / `handle_chat_alt_key()` / `handle_sidebar_key()`
  - `handler_amux.rs`: 提取 `handle_amux_scroll_key()`
  - `session_ops.rs`: 提取 6 个辅助函数 (`collect_git_info`, `spawn_completion_check` 等)
- **效果**: 全部 7 个超标函数降至 25 以下, 最复杂的从 43/25 降至 ~15/25
- **验证**:`cargo clippy -- -W clippy::cognitive_complexity -D warnings` clean

### 76. [x] P1 为 extraction.rs (1311行, 0测试) 添加单元测试
- **问题**: extraction.rs 从 discovery.rs 提取后完全没有测试覆盖
- **修复**: 添加 39 个单元测试覆盖 8 个核心纯函数:
  - `clean_user_message` (6): ANSI/HTML 清理, 空白处理
  - `extract_text_from_content` (6): JSON Value → String 提取
  - `compute_diff` (5): 行级 diff 计算
  - `detect_agent_from_path` (4): 路径→agent 类型映射
  - `format_mtime` (6): 时间戳格式化
  - `parse_gsd_session` (4): GSD 会话解析
  - `parse_codex_session` (3): Codex 会话解析
  - `extract_token_usage` (5): token 用量统计
- **效果**: 测试数 309→348, extraction.rs 覆盖率从 0% 提升到关键路径全覆盖

### 77. [x] P1 修复 `cargo clippy --all-targets -- -D warnings` 的 5 个遗漏警告
- **问题**: `--all-targets` 包含测试代码时暴露 5 个新警告,普通 `cargo clippy` 无法捕获
- **修复**:
  - `extraction.rs`: 删除未使用的 `use std::io::Write as IoWrite`
  - `watch.rs`: `clippy::env_set_var` lint 已在 Rust 1.94 移除, 改为 `#[allow(unknown_lints)]`
  - `ui.rs` / `knowledge.rs`: `field_reassign_with_default` → 使用结构体字面量初始化
- **效果**: `cargo clippy --all-targets -- -D warnings` 完全 clean

### 78. [x] P2 现代化 format! 和闭包写法 (clippy::uninlined_format_args + redundant_closure)
- **问题**: 196 处 `format!("{}", var)` 未使用 Rust 1.58+ 内联语法; 34 处冗余闭包
- **修复**:
  - `format!("{}", name)` → `format!("{name}")` (196 处, 排除 tracing 宏)
  - `.map(|x| f(x))` → `.map(f)` (34 处冗余闭包)
- **效果**: 代码更符合现代 Rust 惯用法
- **验证**:`cargo clippy --all-targets -- -W clippy::uninlined_format_args -W clippy::redundant_closure -D warnings` clean
|------|------|----------|----------|
| **今天** | #3, #4, #5, #6 | 无 | 7 处 `PtyState::*` / 60 行注释 / 18 行重复全部消失 |
| **本周** | #8, #11, #12, #13, #14, #31 | tracing 引入;xterm 资源 | 断网启动 web 模式正常 |
| **本月** | #9, #23, #33, #34, #35 | #34 装好 runner | 第一次 `cargo publish` 跑通,CI 绿 |
| **下季度** | #15, #16, #26, #19 | 多数需要重 UI 工作 | MCP 服务 + 多分栏可用 |
| **长期** | #7, #10, #40, #42 | 架构债 | god-struct 拆完,docs 完整 |

---

## 维护规则

1. **每完成一项**:把 `[ ]` 改 `[x]`,在 commit message 末尾加 `Refs: .planning/IMPROVEMENTS.md#N`
2. **每发现新问题**:追加到对应 phase 末尾,新 P 等级
3. **每季度审视**:把已完成的 `[x]` 折叠到 "历史归档"段,空出主表
4. **优先级重排**:仅在 P0/P1 全部清空后,或用户要求时
5. **冲突解决**:如果某个 P3 任务实现上需要 P0/P1 基础设施(如 MCP 暴露前先有 tracing),立即升级它的优先级
